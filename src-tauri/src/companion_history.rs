//! Companion history: Back/Forward over the live native child's own navigation.
//!
//! WHY this module exists: the pinned JavaScript child-WebView surface exposes
//! no traversal or observation hooks, so in-page website routing (link clicks
//! inside the loaded page) is invisible to the frontend. The toolbar's
//! Back/Forward therefore reach WebView2 directly through the Rust Webview
//! handle — the same proven pattern as the audio owner — reading
//! `CanGoBack`/`CanGoForward` for enable-state, driving `GoBack`/`GoForward`
//! for steps, and forwarding `HistoryChanged`/`NavigationCompleted` as
//! frontend events. The saved-site picker stays the sole site-switching
//! surface; a site switch recreates the child, so its native history starts
//! fresh by construction (ADR-0022 Companion is a single isolated site).
//!
//! Deletion test: removing this module removes native traversal with it —
//! callers keep only the saved-address picker, reload-is-saved, and the
//! browser-preview iframe fallback, never a second traversal copy.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// The frontend event carrying fresh enable-state after any native navigation.
pub const COMPANION_HISTORY_CHANGED_EVENT: &str = "companion-history-changed";

/// The toolbar's Back/Forward picture: what the live native child can step to.
/// Missing child (floating, no URL, preview iframe, failed pane) reads as
/// disabled — never hidden — so enable-state can never desync from history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CompanionHistoryState {
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

impl CompanionHistoryState {
    /// The honest disabled picture: no history means disabled.
    pub fn disabled() -> Self {
        Self {
            can_go_back: false,
            can_go_forward: false,
        }
    }
}

/// The current enable-state of the live native child. Best effort like the
/// audio read: a missing child or a failed round trip reads as disabled.
pub fn current_state(app: &AppHandle) -> CompanionHistoryState {
    read_live(app).unwrap_or_else(CompanionHistoryState::disabled)
}

/// Steps the live native child back through its in-page history, then reports
/// the fresh enable-state. A missing child is refused plainly instead of
/// navigating nowhere.
pub fn go_back(app: &AppHandle) -> Result<CompanionHistoryState, String> {
    step(app, true)
}

/// Steps the live native child forward through its in-page history, then
/// reports the fresh enable-state.
pub fn go_forward(app: &AppHandle) -> Result<CompanionHistoryState, String> {
    step(app, false)
}

/// Attaches the native history observers to the live child (idempotent per
/// child) and returns the current enable-state. The observers forward every
/// in-page navigation as [`COMPANION_HISTORY_CHANGED_EVENT`] so the toolbar
/// follows link clicks without polling. Best effort: a missing child returns
/// the disabled picture and attaches nothing.
pub fn ensure_history_hook(app: &AppHandle) -> Result<CompanionHistoryState, String> {
    #[cfg(not(windows))]
    {
        let _ = app;
        return Ok(CompanionHistoryState::disabled());
    }
    #[cfg(windows)]
    {
        attach_hook(app);
        Ok(current_state(app))
    }
}

#[cfg(windows)]
fn step(app: &AppHandle, back: bool) -> Result<CompanionHistoryState, String> {
    let webview = app
        .get_webview(crate::companion_audio::COMPANION_WEBVIEW_LABEL)
        .ok_or_else(|| {
            "Companion is not showing a site right now — pick a saved site first.".to_string()
        })?;
    let (tx, rx) = std::sync::mpsc::channel();
    let direction = if back { "back" } else { "forward" };
    webview
        .with_webview(move |platform| {
            let result = step_on_platform(&platform, back).map_err(|e| {
                format!("Couldn't go {direction} in the companion page — {e}")
            });
            let _ = tx.send(result);
        })
        .map_err(|e| format!("Couldn't reach the companion page — {e}"))?;
    match rx.recv_timeout(std::time::Duration::from_millis(1000)) {
        Ok(Ok(())) => Ok(current_state(app)),
        Ok(Err(message)) => Err(message),
        Err(_) => Err(format!(
            "Couldn't go {direction} in the companion page — it didn't answer in time."
        )),
    }
}

#[cfg(not(windows))]
fn step(_app: &AppHandle, back: bool) -> Result<CompanionHistoryState, String> {
    let direction = if back { "back" } else { "forward" };
    Err(format!(
        "Couldn't go {direction} in the companion page — history needs the Windows app."
    ))
}

#[cfg(windows)]
fn read_live(app: &AppHandle) -> Option<CompanionHistoryState> {
    let webview = app.get_webview(crate::companion_audio::COMPANION_WEBVIEW_LABEL)?;
    let (tx, rx) = std::sync::mpsc::channel();
    // WHY mpsc: with_webview runs its closure on the main thread, so the
    // value has to travel back across threads (same shape as the audio read).
    webview
        .with_webview(move |platform| {
            let _ = tx.send(state_from_platform(&platform));
        })
        .ok()?;
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .ok()?
}

#[cfg(windows)]
fn state_from_platform(platform: &tauri::webview::PlatformWebview) -> Option<CompanionHistoryState> {
    unsafe {
        let core = platform.controller().CoreWebView2().ok()?;
        let mut back_raw: i32 = 0;
        core.CanGoBack(&mut back_raw as *mut i32 as *mut _).ok()?;
        let mut forward_raw: i32 = 0;
        core.CanGoForward(&mut forward_raw as *mut i32 as *mut _)
            .ok()?;
        Some(CompanionHistoryState {
            can_go_back: back_raw != 0,
            can_go_forward: forward_raw != 0,
        })
    }
}

#[cfg(windows)]
fn step_on_platform(
    platform: &tauri::webview::PlatformWebview,
    back: bool,
) -> Result<(), String> {
    unsafe {
        let core = platform
            .controller()
            .CoreWebView2()
            .map_err(|e| e.to_string())?;
        if back {
            core.GoBack().map_err(|e| e.to_string())
        } else {
            core.GoForward().map_err(|e| e.to_string())
        }
    }
}

/// Attaches `HistoryChanged` + `NavigationCompleted` forwarding to the live
/// child. Previously attached tokens are removed first so repeated calls
/// (creation, switch, retry, poll recovery) never stack duplicate emitters on
/// the same child; tokens from a destroyed child fail removal harmlessly.
#[cfg(windows)]
fn attach_hook(app: &AppHandle) {
    let Some(webview) = app.get_webview(crate::companion_audio::COMPANION_WEBVIEW_LABEL) else {
        return;
    };
    let app_for_emit = app.clone();
    let _ = webview.with_webview(move |platform| {
        unsafe {
            let Ok(core) = platform.controller().CoreWebView2() else {
                return;
            };
            // Drop any previous observers on this child before re-attaching.
            if let Some((history_token, nav_token)) = take_hooked_tokens() {
                let _ = core.remove_HistoryChanged(history_token);
                let _ = core.remove_NavigationCompleted(nav_token);
            }
            let emit_app = app_for_emit.clone();
            let history_handler =
                webview2_com::HistoryChangedEventHandler::create(Box::new(
                    move |core_opt, _| {
                        if let Some(core) = core_opt {
                            let mut back_raw: i32 = 0;
                            let mut forward_raw: i32 = 0;
                            let _ = core.CanGoBack(&mut back_raw as *mut i32 as *mut _);
                            let _ = core.CanGoForward(
                                &mut forward_raw as *mut i32 as *mut _,
                            );
                            let _ = emit_app.emit(
                                COMPANION_HISTORY_CHANGED_EVENT,
                                &CompanionHistoryState {
                                    can_go_back: back_raw != 0,
                                    can_go_forward: forward_raw != 0,
                                },
                            );
                        }
                        Ok(())
                    },
                ));
            let mut history_token: i64 = 0;
            let history_attached = core
                .add_HistoryChanged(&history_handler, &mut history_token)
                .is_ok();
            let emit_app = app_for_emit.clone();
            let nav_handler =
                webview2_com::NavigationCompletedEventHandler::create(Box::new(
                    move |core_opt, _| {
                        if let Some(core) = core_opt {
                            let mut back_raw: i32 = 0;
                            let mut forward_raw: i32 = 0;
                            let _ = core.CanGoBack(&mut back_raw as *mut i32 as *mut _);
                            let _ = core.CanGoForward(
                                &mut forward_raw as *mut i32 as *mut _,
                            );
                            let _ = emit_app.emit(
                                COMPANION_HISTORY_CHANGED_EVENT,
                                &CompanionHistoryState {
                                    can_go_back: back_raw != 0,
                                    can_go_forward: forward_raw != 0,
                                },
                            );
                        }
                        Ok(())
                    },
                ));
            let mut nav_token: i64 = 0;
            let nav_attached = core
                .add_NavigationCompleted(&nav_handler, &mut nav_token)
                .is_ok();
            if history_attached && nav_attached {
                store_hooked_tokens(history_token, nav_token);
            }
        }
    });
}

#[cfg(windows)]
static HOOKED_TOKENS: std::sync::Mutex<Option<(i64, i64)>> =
    std::sync::Mutex::new(None);

#[cfg(windows)]
fn take_hooked_tokens() -> Option<(i64, i64)> {
    HOOKED_TOKENS.lock().ok()?.take()
}

#[cfg(windows)]
fn store_hooked_tokens(history_token: i64, nav_token: i64) {
    if let Ok(mut slot) = HOOKED_TOKENS.lock() {
        *slot = Some((history_token, nav_token));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_state_is_honestly_disabled() {
        let state = CompanionHistoryState::disabled();
        assert!(!state.can_go_back);
        assert!(!state.can_go_forward);
    }

    #[test]
    fn history_event_name_is_stable_for_the_frontend() {
        assert_eq!(COMPANION_HISTORY_CHANGED_EVENT, "companion-history-changed");
    }

    #[test]
    fn history_state_serializes_with_snake_case_flags() {
        let state = CompanionHistoryState {
            can_go_back: true,
            can_go_forward: false,
        };
        let value = serde_json::to_value(&state).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "can_go_back": true, "can_go_forward": false })
        );
    }

    #[test]
    fn disabled_state_round_trips_through_json() {
        let state = CompanionHistoryState::disabled();
        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"can_go_back\":false"));
        assert!(json.contains("\"can_go_forward\":false"));
    }
}
