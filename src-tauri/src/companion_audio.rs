//! Companion audio: the persisted mute plus the live WebView2 read/apply.
//!
//! WHY this module exists: Tauri's JS Webview surface has no audio API, so
//! the toolbar's mute toggle and playing indicator reach WebView2 directly
//! through the Rust Webview handle. The persisted mute is the source of
//! truth; the live WebView is healed toward it on every read, so a recreated
//! pane (navigation, redock, restart) can never come back loud after silence
//! was asked for.

use serde::Serialize;
use tauri::{AppHandle, Manager};

/// The toolbar's audio picture: persisted mute plus live playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CompanionAudioState {
    pub muted: bool,
    pub playing: bool,
}

/// The child WebView label the dock frontend creates.
pub const COMPANION_WEBVIEW_LABEL: &str = "companion";

/// Reads the persisted mute without touching any WebView.
pub fn persisted_muted(app: &AppHandle) -> bool {
    let Some(state) = app.try_state::<crate::AppState>() else {
        return crate::settings::DEFAULT_COMPANION_MUTED;
    };
    let Ok(conn) = state.db.lock() else {
        return crate::settings::DEFAULT_COMPANION_MUTED;
    };
    crate::settings::load_companion_muted(&conn)
}

/// The current toolbar state: persisted mute plus live playback. Heals a
/// drifted WebView toward the persisted mute, so a fresh pane never stays
/// loud after silence was asked for.
pub fn current_state(app: &AppHandle, persisted: bool) -> CompanionAudioState {
    match read_live(app) {
        Some((_live_muted, live_playing)) => {
            if _live_muted != persisted {
                apply_muted(app, persisted);
            }
            CompanionAudioState {
                muted: persisted,
                playing: live_playing,
            }
        }
        None => CompanionAudioState {
            muted: persisted,
            playing: false,
        },
    }
}

/// Pushes the persisted mute into the live WebView. No-op while the pane has
/// no native surface (floating, no URL, preview iframe).
pub fn apply_muted(app: &AppHandle, muted: bool) {
    let Some(webview) = app.get_webview(COMPANION_WEBVIEW_LABEL) else {
        return;
    };
    let _ = webview.with_webview(move |platform| {
        apply_to_platform(&platform, muted);
    });
}

/// Best-effort live read of (muted, playing). `None` while the pane has no
/// native surface or the main-thread round trip fails.
fn read_live(app: &AppHandle) -> Option<(bool, bool)> {
    let webview = app.get_webview(COMPANION_WEBVIEW_LABEL)?;
    let (tx, rx) = std::sync::mpsc::channel();
    // WHY mpsc: with_webview runs its closure on the main thread, so the
    // value has to travel back across threads.
    webview
        .with_webview(move |platform| {
            let _ = tx.send(live_from_platform(&platform));
        })
        .ok()?;
    rx.recv_timeout(std::time::Duration::from_millis(500))
        .ok()?
}

#[cfg(windows)]
fn live_from_platform(platform: &tauri::webview::PlatformWebview) -> Option<(bool, bool)> {
    use windows_core::Interface;
    // WHY the i32 dance: the COM getters take *mut BOOL without naming a
    // versioned windows type here, and BOOL is a 4-byte transparent wrapper —
    // reading through an i32 slot keeps this crate off the windows version
    // the bindings were generated against.
    unsafe {
        let core = platform.controller().CoreWebView2().ok()?;
        let v8: webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8 =
            core.cast().ok()?;
        let mut muted_raw: i32 = 0;
        v8.IsMuted(&mut muted_raw as *mut i32 as *mut _).ok()?;
        let mut playing_raw: i32 = 0;
        v8.IsDocumentPlayingAudio(&mut playing_raw as *mut i32 as *mut _)
            .ok()?;
        Some((muted_raw != 0, playing_raw != 0))
    }
}

#[cfg(not(windows))]
fn live_from_platform(_platform: &tauri::webview::PlatformWebview) -> Option<(bool, bool)> {
    None
}

#[cfg(windows)]
fn apply_to_platform(platform: &tauri::webview::PlatformWebview, muted: bool) {
    use windows_core::Interface;
    unsafe {
        let Ok(core) = platform.controller().CoreWebView2() else {
            return;
        };
        let Ok(v8) =
            core.cast::<webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2_8>()
        else {
            return;
        };
        let _ = v8.SetIsMuted(muted);
    }
}

#[cfg(not(windows))]
fn apply_to_platform(_platform: &tauri::webview::PlatformWebview, _muted: bool) {}
