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

/// The main window's default inner size (mirrors `tauri.conf.json`).
pub const MAIN_WINDOW_WIDTH: f64 = 1200.0;
pub const MAIN_WINDOW_HEIGHT: f64 = 800.0;

/// The main window's minimum inner size (mirrors `tauri.conf.json`).
pub const MAIN_WINDOW_MIN_WIDTH: f64 = 900.0;
pub const MAIN_WINDOW_MIN_HEIGHT: f64 = 620.0;