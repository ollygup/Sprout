//! Shared UI-geometry constants (spec 55, ticket 56). Every reusable window
//! dimension lives here — never re-declared in another module. Scan this file
//! first before any UI-dimension change (AGENTS.md design rule).

/// The Quick Launch window's one and only width (physical pixels): the fixed
/// floating palette size and the docked strip's width are the same value, so
/// the dock → undock round trip is lossless.
pub const WINDOW_WIDTH: u32 = 340;

/// The Quick Launch window's fixed height (physical pixels).
pub const WINDOW_HEIGHT: u32 = 460;

/// The docked strip's width — exactly the floating window's width. Deleting
/// the old 320 in `appbar.rs` is what fixes the "undock leaves the window
/// smaller" bug: there is only one width source now.
pub const DOCK_WIDTH: u32 = WINDOW_WIDTH;

/// The auto-hide sliver's width in physical pixels (ticket 63): how much of
/// the docked strip remains on-screen while hidden — the grab handle the
/// cursor can still reach, taskbar-style.
pub const AUTOHIDE_SLIVER_PX: i32 = 2;

/// The edge trigger band's width in physical pixels (ticket 63): how close
/// to the docked screen edge the cursor must come to reveal a hidden strip.
pub const EDGE_TRIGGER_PX: i32 = 8;

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
