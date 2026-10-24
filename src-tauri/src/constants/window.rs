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
/// effective pixel width is `dock_width_px_for_mode` below — never below
/// [`DOCK_WIDTH`], never above the current mode's cap. The floating window
/// ignores it entirely (stays [`WINDOW_WIDTH`]).
///
/// Cap math (in-ticket research, ticket 128; overlay-vs-reservation policy in
/// research 0015, ADR-0011, ADR-0021): 60% of a 3440px ultrawide is
/// 2064px as a *fixed* AppBar — wider than a whole 1080p monitor, leaving
/// only 1376px for apps — extreme for a strip that permanently reserves
/// workspace. 30% keeps the reservation at most a third of any screen
/// (1920→576, 2560→768, 3440→1032, 5120→1536): meaningfully wider than
/// today's 340 for long names, still a strip, and the auto-hide slide stays
/// short. Auto-hide overlays instead of reserving (ADR-0011), so the
/// reservation objection does not apply to it — it may run to 60%.
pub const DOCK_WIDTH_MIN_PCT: u32 = 10;
/// Fixed keeps the reserving-strip cap (ADR-0011 fixed reserves workspace).
pub const DOCK_WIDTH_MAX_PCT_FIXED: u32 = 30;
/// Auto-hide overlays content and reserves nothing, so it may run wide.
pub const DOCK_WIDTH_MAX_PCT_AUTOHIDE: u32 = 60;
/// Bezel overlays content and reserves nothing while collapsed, so its open
/// panel may run wide like auto-hide (ADR-0011 overlay vs reservation).
pub const DOCK_WIDTH_MAX_PCT_BEZEL: u32 = 60;
/// The widest % any mode stores — the Settings and per-monitor validation
/// range. Kept equal to the auto-hide cap; fixed clamps into its own cap on
/// apply, so a stored 60 never explodes a fixed strip.
pub const DOCK_WIDTH_MAX_PCT_ABSOLUTE: u32 = DOCK_WIDTH_MAX_PCT_AUTOHIDE;
/// ~346px on a 1920 reference monitor — the closest whole % to today's 340.
pub const DOCK_WIDTH_DEFAULT_PCT: u32 = 18;

/// The mode's width cap as % of the monitor: fixed stays a strip, auto-hide
/// and bezel may overlay wide (ADR-0011 overlay vs reservation; ADR-0021
/// single size source). Unknown modes take the fixed cap — a reserving
/// assumption never over-claims workspace.
pub fn dock_width_max_pct_for_mode(mode: &str) -> u32 {
    match mode {
        "auto-hide" => DOCK_WIDTH_MAX_PCT_AUTOHIDE,
        "bezel" => DOCK_WIDTH_MAX_PCT_BEZEL,
        _ => DOCK_WIDTH_MAX_PCT_FIXED,
    }
}

/// The effective docked-strip width in physical pixels for a monitor
/// `monitor_width_px` wide at `pct` % in `mode`: `% of monitor, floored at
/// today's width and capped at the mode's cap`. Pure — the single width
/// derivation every dock placement shares. `pct` outside the absolute range
/// clamps into it first, then into the mode's cap, so a broken stored value
/// can never collapse or explode the strip; a degenerate monitor width falls
/// back to the floor.
pub fn dock_width_px_for_mode(monitor_width_px: i32, pct: u32, mode: &str) -> i32 {
    if monitor_width_px <= 0 {
        return DOCK_WIDTH as i32;
    }
    let max = dock_width_max_pct_for_mode(mode);
    let pct = pct.clamp(DOCK_WIDTH_MIN_PCT, DOCK_WIDTH_MAX_PCT_ABSOLUTE).min(max);
    let cap = monitor_width_px * max as i32 / 100;
    let want = monitor_width_px * pct as i32 / 100;
    want.clamp(DOCK_WIDTH as i32, cap.max(DOCK_WIDTH as i32))
}

/// The bezel collapsed tab's visible width in physical pixels: wide enough
/// to hit reliably on a screen edge (ADR-0011 third mode) — 14px read as
/// unclickable in practice, so the tab keeps its quiet strip while clearing
/// a pointer hit threshold.
pub const BEZEL_COLLAPSED_WIDTH_PX: i32 = 20;
/// The bezel hover-peek width in physical pixels: geometry-only feedback —
/// peek never opens (ADR-0019 bezel amendment; open stays click-gated). Three
/// times the collapsed tab, so the hover target is forgiving and the
/// anticipation reads before any commitment.
pub const BEZEL_PEEK_WIDTH_PX: i32 = 60;
/// The bezel tab height as a ratio of the docked monitor's full height
/// (spec 214): monitor-relative, never a fixed pixel height.
pub const BEZEL_HEIGHT_RATIO: f64 = 0.12;
/// Physical-px clamps for the ratio-derived tab height.
pub const BEZEL_HEIGHT_MIN_PX: i32 = 64;
pub const BEZEL_HEIGHT_MAX_PX: i32 = 160;
/// The default Y position as a ratio of the tab's travel (0 = top, 1 =
/// bottom): centered until the user drags it (Y persists per display).
pub const BEZEL_Y_RATIO_DEFAULT: f64 = 0.5;

/// The bezel tab height in physical pixels for a monitor `monitor_height_px`
/// tall: 12% of the monitor, clamped to 64–160px. Pure — the single
/// tab-height derivation the collapsed/peek rects and the frontend command
/// share. A degenerate monitor height falls back to the floor, never zero.
pub fn bezel_tab_height_px(monitor_height_px: i32) -> i32 {
    if monitor_height_px <= 0 {
        return BEZEL_HEIGHT_MIN_PX;
    }
    let want = (monitor_height_px as f64 * BEZEL_HEIGHT_RATIO).round() as i32;
    want.clamp(BEZEL_HEIGHT_MIN_PX, BEZEL_HEIGHT_MAX_PX)
}

/// Clamps a stored bezel Y ratio into 0..1: a broken stored value centers
/// instead of parking the tab off-screen. Non-finite reads as the default —
/// clamping NaN would be meaningless.
pub fn clamp_bezel_y_ratio(ratio: f64) -> f64 {
    if !ratio.is_finite() {
        return BEZEL_Y_RATIO_DEFAULT;
    }
    ratio.clamp(0.0, 1.0)
}

/// The bezel tab's top edge in physical pixels: `y_ratio` of the tab's travel
/// (`monitor_height_px - tab_height_px`) below `monitor_top_px`. Pure.
pub fn bezel_tab_y_px(
    monitor_top_px: i32,
    monitor_height_px: i32,
    tab_height_px: i32,
    y_ratio: f64,
) -> i32 {
    let travel = (monitor_height_px - tab_height_px).max(0);
    monitor_top_px + ((travel as f64 * clamp_bezel_y_ratio(y_ratio)).round() as i32)
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

/// The clip viewer's preferred inner size: the default is the
/// smaller of this and 80% of the invoking monitor's work area.
pub const CLIP_VIEWER_DEFAULT_WIDTH: u32 = 960;
pub const CLIP_VIEWER_DEFAULT_HEIGHT: u32 = 720;

/// The clip viewer's minimum inner size: a remembered size never
/// collapses below this unless the work area itself is smaller.
pub const CLIP_VIEWER_MIN_WIDTH: u32 = 480;
pub const CLIP_VIEWER_MIN_HEIGHT: u32 = 360;

/// The share of the work area the clip viewer's default size may take.
pub const CLIP_VIEWER_WORK_AREA_PCT: u32 = 80;

/// The clip viewer's default size in physical pixels for a monitor work area
/// `work_width_px` × `work_height_px`: `min(960×720, 80% of the work area)`,
/// floored at 480×360 — unless the work area is smaller than that floor, which
/// always wins so the window still fits on screen. Pure — the single default
/// derivation the viewer's open path uses. A degenerate work area falls back
/// to the preferred size.
pub fn clip_viewer_default_size(work_width_px: i32, work_height_px: i32) -> (u32, u32) {
    if work_width_px <= 0 || work_height_px <= 0 {
        return (CLIP_VIEWER_DEFAULT_WIDTH, CLIP_VIEWER_DEFAULT_HEIGHT);
    }
    let share = |work: i32| -> u32 {
        ((work as u64 * CLIP_VIEWER_WORK_AREA_PCT as u64) / 100) as u32
    };
    let fit = |work: i32, preferred: u32, minimum: u32| -> u32 {
        share(work)
            .min(preferred)
            .max(minimum)
            .min(work as u32)
    };
    (
        fit(work_width_px, CLIP_VIEWER_DEFAULT_WIDTH, CLIP_VIEWER_MIN_WIDTH),
        fit(work_height_px, CLIP_VIEWER_DEFAULT_HEIGHT, CLIP_VIEWER_MIN_HEIGHT),
    )
}

/// Clamps a remembered clip-viewer size into the current work area: never
/// below 480×360 and never larger than the work area, with the
/// work area winning when it is itself smaller than the floor. A broken
/// (zero, negative) stored pair falls back to the default derivation. Pure.
pub fn clip_viewer_clamp_size(
    width: i64,
    height: i64,
    work_width_px: i32,
    work_height_px: i32,
) -> (u32, u32) {
    if width <= 0 || height <= 0 || work_width_px <= 0 || work_height_px <= 0 {
        return clip_viewer_default_size(work_width_px, work_height_px);
    }
    let clamp = |value: i64, work: i32, minimum: u32| -> u32 {
        (value.min(work as i64).max(0) as u32)
            .max(minimum)
            .min(work as u32)
    };
    (
        clamp(width, work_width_px, CLIP_VIEWER_MIN_WIDTH),
        clamp(height, work_height_px, CLIP_VIEWER_MIN_HEIGHT),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dock_width_floors_at_today_width_and_caps_at_30pct() {
        // Fixed keeps the reserving-strip behavior (ADR-0011).
        assert_eq!(dock_width_px_for_mode(1920, 18, "fixed"), 345); // 1920*18/100
        assert_eq!(dock_width_px_for_mode(1920, 10, "fixed"), DOCK_WIDTH as i32); // 192→floor
        assert_eq!(dock_width_px_for_mode(1920, 30, "fixed"), 576);
        assert_eq!(dock_width_px_for_mode(2560, 30, "fixed"), 768);
        assert_eq!(dock_width_px_for_mode(3440, 30, "fixed"), 1032);
        // Broken % clamps into range first: 5→10→floor, 99→30→cap.
        assert_eq!(dock_width_px_for_mode(1920, 5, "fixed"), DOCK_WIDTH as i32);
        assert_eq!(dock_width_px_for_mode(1920, 99, "fixed"), 576);
        // A degenerate monitor never collapses the strip.
        assert_eq!(dock_width_px_for_mode(0, 18, "fixed"), DOCK_WIDTH as i32);
    }

    #[test]
    fn dock_width_splits_caps_by_mode() {
        // Fixed stays a strip (ADR-0011 reservation); auto-hide may overlay wide.
        assert_eq!(dock_width_max_pct_for_mode("fixed"), 30);
        assert_eq!(dock_width_max_pct_for_mode("auto-hide"), 60);
        assert_eq!(dock_width_max_pct_for_mode("bogus"), 30);
        // Mode-aware: fixed clamps 45→30, auto-hide honors to 60.
        assert_eq!(dock_width_px_for_mode(1920, 45, "fixed"), 576);
        assert_eq!(dock_width_px_for_mode(1920, 45, "auto-hide"), 864);
        assert_eq!(dock_width_px_for_mode(1920, 60, "auto-hide"), 1152);
        assert_eq!(dock_width_px_for_mode(3440, 60, "auto-hide"), 2064);
        assert_eq!(dock_width_px_for_mode(1920, 99, "auto-hide"), 1152);
        assert_eq!(dock_width_px_for_mode(1920, 5, "auto-hide"), DOCK_WIDTH as i32);
        assert_eq!(dock_width_px_for_mode(0, 45, "auto-hide"), DOCK_WIDTH as i32);
    }

    #[test]
    fn bezel_open_caps_like_an_overlay_at_60pct() {
        // Bezel reserves nothing extra, so open overlays wide (ADR-0021 bezel
        // amendment) — the same cap as auto-hide, never the fixed strip cap.
        assert_eq!(dock_width_max_pct_for_mode("bezel"), 60);
        assert_eq!(dock_width_px_for_mode(1920, 45, "bezel"), 864);
        assert_eq!(dock_width_px_for_mode(1920, 60, "bezel"), 1152);
        // Broken % clamps into range first: 99→60→cap, 5→10→floor.
        assert_eq!(dock_width_px_for_mode(1920, 99, "bezel"), 1152);
        assert_eq!(dock_width_px_for_mode(1920, 5, "bezel"), DOCK_WIDTH as i32);
        // A degenerate monitor never collapses the panel.
        assert_eq!(dock_width_px_for_mode(0, 45, "bezel"), DOCK_WIDTH as i32);
    }

    #[test]
    fn bezel_tab_height_follows_monitor_ratio_with_clamps() {
        // 12% of the monitor, clamped to 64–160 physical px (ADR-0021 bezel
        // amendment) — monitor-relative, never a fixed pixel height.
        assert_eq!(bezel_tab_height_px(1080), 130); // 129.6 rounds to 130
        assert_eq!(bezel_tab_height_px(2160), 160); // 259.2 clamps to the ceiling
        assert_eq!(bezel_tab_height_px(400), 64); // 48.0 clamps to the floor
        // A degenerate monitor never collapses the tab.
        assert_eq!(bezel_tab_height_px(0), BEZEL_HEIGHT_MIN_PX);
        assert_eq!(bezel_tab_height_px(-5), BEZEL_HEIGHT_MIN_PX);
    }

    #[test]
    fn bezel_y_ratio_clamps_and_broken_values_center() {
        assert_eq!(clamp_bezel_y_ratio(0.5), 0.5);
        assert_eq!(clamp_bezel_y_ratio(0.0), 0.0);
        assert_eq!(clamp_bezel_y_ratio(1.0), 1.0);
        assert_eq!(clamp_bezel_y_ratio(-0.2), 0.0);
        assert_eq!(clamp_bezel_y_ratio(1.4), 1.0);
        // Non-finite stored values center — clamping NaN would be meaningless.
        assert_eq!(clamp_bezel_y_ratio(f64::NAN), BEZEL_Y_RATIO_DEFAULT);
        assert_eq!(clamp_bezel_y_ratio(f64::INFINITY), BEZEL_Y_RATIO_DEFAULT);
    }

    #[test]
    fn bezel_tab_y_centers_by_default_within_travel() {
        // 1080-tall monitor, 130-tall tab: travel is 950, centered top is 475.
        assert_eq!(bezel_tab_y_px(0, 1080, 130, 0.5), 475);
        assert_eq!(bezel_tab_y_px(0, 1080, 130, 0.0), 0);
        assert_eq!(bezel_tab_y_px(0, 1080, 130, 1.0), 950);
        // A non-zero monitor origin rides along; broken ratios center.
        assert_eq!(bezel_tab_y_px(100, 1080, 130, 0.5), 575);
        assert_eq!(bezel_tab_y_px(0, 1080, 130, f64::NAN), 475);
        // A tab taller than the monitor parks at the top instead of underflowing.
        assert_eq!(bezel_tab_y_px(0, 100, 160, 0.5), 0);
    }

    #[test]
    fn clip_viewer_default_is_smaller_of_preferred_and_80pct() {
        // A roomy work area keeps the preferred 960×720.
        assert_eq!(clip_viewer_default_size(1920, 1040), (960, 720));
        assert_eq!(clip_viewer_default_size(2560, 1400), (960, 720));
        assert_eq!(clip_viewer_default_size(5120, 2160), (960, 720));
        // A smaller work area takes 80% of each axis.
        assert_eq!(clip_viewer_default_size(1024, 768), (819, 614));
        assert_eq!(clip_viewer_default_size(1280, 960), (960, 720));
        assert_eq!(clip_viewer_default_size(1600, 900), (960, 720));
    }

    #[test]
    fn clip_viewer_default_respects_floor_and_tiny_work_areas() {
        // The 480×360 floor wins over 80% of a small work area…
        assert_eq!(clip_viewer_default_size(600, 400), (480, 360));
        // …but the work area itself wins when it is smaller than the floor.
        assert_eq!(clip_viewer_default_size(400, 300), (400, 300));
        // A degenerate probe falls back to the preferred size, never zero.
        assert_eq!(clip_viewer_default_size(0, 0), (960, 720));
        assert_eq!(clip_viewer_default_size(-10, 800), (960, 720));
    }

    #[test]
    fn clip_viewer_remembered_size_clamps_to_floor_and_work_area() {
        // A size that fits is kept exactly.
        assert_eq!(clip_viewer_clamp_size(1200, 900, 1920, 1040), (1200, 900));
        // Below the floor grows to 480×360.
        assert_eq!(clip_viewer_clamp_size(200, 100, 1920, 1040), (480, 360));
        // Larger than the work area shrinks to it.
        assert_eq!(clip_viewer_clamp_size(4000, 3000, 1920, 1040), (1920, 1040));
        // A broken stored pair reads as the default.
        assert_eq!(clip_viewer_clamp_size(0, 0, 1920, 1040), (960, 720));
        assert_eq!(clip_viewer_clamp_size(-4, 900, 1920, 1040), (960, 720));
        // The work area wins when it is smaller than the floor.
        assert_eq!(clip_viewer_clamp_size(800, 600, 400, 300), (400, 300));
    }
}
