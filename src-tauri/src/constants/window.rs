//! Shared UI-geometry constants (spec 55, ticket 56). Every reusable window
//! dimension lives here — never re-declared in another module. Scan this file
//! first before any UI-dimension change (AGENTS.md design rule).

/// The Quick Launch window's fixed floating size (physical pixels): the
/// palette stays 340×460 wherever it floats — width is never draggable, so a
/// wide dock always restores to exactly this on undock.
pub const WINDOW_WIDTH: u32 = 340;

/// The Quick Launch window's fixed height (physical pixels).
pub const WINDOW_HEIGHT: u32 = 460;

/// The docked strip's minimum width — exactly the floating window's width.
/// The dock is never narrower than the palette it restores to, so the
/// dock → undock round trip never shrinks the window; it only ever narrows a
/// widened dock back to this floor (ticket 128).
pub const DOCK_WIDTH: u32 = WINDOW_WIDTH;

/// The docked strip's width as % of its monitor's full width (ticket 128):
/// the Settings slider writes this, per-monitor memory overrides it, and the
/// effective pixel width is `dock_width_px` below — never below [`DOCK_WIDTH`],
/// never above [`DOCK_WIDTH_MAX_PCT`] of the monitor. The floating window
/// ignores it entirely (stays [`WINDOW_WIDTH`]).
///
/// Cap math (in-ticket research, ticket 128): 60% of a 3440px ultrawide is
/// 2064px as a *fixed* AppBar — wider than a whole 1080p monitor, leaving
/// only 1376px for apps — extreme for a strip that permanently reserves
/// workspace. 30% keeps the reservation at most a third of any screen
/// (1920→576, 2560→768, 3440→1032, 5120→1536): meaningfully wider than
/// today's 340 for long names, still a strip, and the auto-hide slide stays
/// short.
pub const DOCK_WIDTH_MIN_PCT: u32 = 10;
pub const DOCK_WIDTH_MAX_PCT: u32 = 30;
/// ~346px on a 1920 reference monitor — the closest whole % to today's 340.
pub const DOCK_WIDTH_DEFAULT_PCT: u32 = 18;

/// The effective docked-strip width in physical pixels for a monitor
/// `monitor_width_px` wide at `pct` %: `% of monitor, floored at today's
/// width and capped at [`DOCK_WIDTH_MAX_PCT`] of the monitor`. Pure — the
/// single width derivation every dock placement shares. `pct` outside the
/// slider range clamps into it first, so a broken stored value can never
/// collapse or explode the strip; a degenerate monitor width falls back to
/// the floor.
pub fn dock_width_px(monitor_width_px: i32, pct: u32) -> i32 {
    if monitor_width_px <= 0 {
        return DOCK_WIDTH as i32;
    }
    let pct = pct.clamp(DOCK_WIDTH_MIN_PCT, DOCK_WIDTH_MAX_PCT);
    let cap = monitor_width_px * DOCK_WIDTH_MAX_PCT as i32 / 100;
    let want = monitor_width_px * pct as i32 / 100;
    want.clamp(DOCK_WIDTH as i32, cap.max(DOCK_WIDTH as i32))
}

/// The auto-hide sliver's width in physical pixels (ticket 63 — kept only
/// for the integer trigger-band math; ticket 119 hides off-screen with no
/// handle, so this is the band width at the wall, not a visible strip).
pub const AUTOHIDE_SLIVER_PX: i32 = 2;

/// The reveal dwell in milliseconds (ticket 112): how long the cursor must
/// stay inside the sliver band after accumulating sufficient toward-edge
/// travel before the dock reveals. Any exit cancels instantly.
pub const REVEAL_DWELL_MS: u64 = 200;

/// The toward-edge travel threshold in physical pixels (ticket 112):
/// accumulated toward-edge motion inside the sliver must exceed this before
/// the dwell starts. Samples dominated by along-edge motion (dy > dx_toward)
/// accumulate nothing.
pub const REVEAL_SENSITIVITY_PX: i32 = 12;

/// Per-sample cap for toward-edge accumulation (ticket 112, GNOME
/// PressureBarrier prior art): prevents a single huge jump from instantly
/// crossing the sensitivity threshold.
pub const REVEAL_MAX_STEP_PX: i32 = 15;

/// The auto-hide driver's poll interval in milliseconds (ticket 63): cursor
/// polling drives hover detection — the WebView2 child HWND swallows mouse
/// messages, so message-driven detection cannot work.
pub const AUTOHIDE_POLL_MS: u64 = 16;

/// The auto-hide driver's poll interval while a slide is animating (ticket
/// 63): with the 1 ms timer resolution raised, this paces the eased motion at
/// display-like ~60 fps — asking a WebView2 window to move faster than it can
/// composite only queues jerky frames.
pub const AUTOHIDE_ANIM_POLL_MS: u64 = 16;

/// The auto-hide slide duration in milliseconds (ticket 63): one direction of
/// the motion (out or away) completes in about this long, eased.
pub const AUTOHIDE_SLIDE_MS: u64 = 180;

/// The main window's default inner size — the single size source: the
/// programmatic build (`lib.rs`'s `open_main_window`) sizes from these
/// constants since the conf file stopped declaring windows (ticket 76,
/// ADR-0013).
pub const MAIN_WINDOW_WIDTH: f64 = 1200.0;
pub const MAIN_WINDOW_HEIGHT: f64 = 800.0;

/// The main window's minimum inner size (single size source, ticket 76).
pub const MAIN_WINDOW_MIN_WIDTH: f64 = 900.0;
pub const MAIN_WINDOW_MIN_HEIGHT: f64 = 620.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_width_floors_at_today_width_and_caps_at_30pct() {
        // Ticket 128: % of monitor, floored at 340, capped at 30%.
        assert_eq!(dock_width_px(1920, 18), 345); // 1920*18/100
        assert_eq!(dock_width_px(1920, 10), DOCK_WIDTH as i32); // 192→floor
        assert_eq!(dock_width_px(1920, 30), 576);
        assert_eq!(dock_width_px(2560, 30), 768);
        assert_eq!(dock_width_px(3440, 30), 1032);
        // Broken % clamps into range first: 5→10→floor, 99→30→cap.
        assert_eq!(dock_width_px(1920, 5), DOCK_WIDTH as i32);
        assert_eq!(dock_width_px(1920, 99), 576);
        // A degenerate monitor never collapses the strip.
        assert_eq!(dock_width_px(0, 18), DOCK_WIDTH as i32);
    }
}
