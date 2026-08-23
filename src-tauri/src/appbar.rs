//! Win32 AppBar docking for the Quick Launch window (ticket 53).
//!
//! The docked form is a taskbar-style AppBar registered with `ABM_NEW` and
//! released with `ABM_REMOVE`. Auto-hide (the default) is Sprout-driven
//! (ticket 63): `ABM_SETAUTOHIDEBAR` buys exclusivity, z-order, notifications
//! and work-area semantics — never motion; no OS mechanism slides an appbar
//! (`docs/research/0003-appbar-autohide-os-contract.md`). The driver in
//! `quick_window` animates the strip between its full rect and a sliver using
//! the pure geometry below ([`sliver_rect`], [`edge_hit`], [`strip_contains`],
//! [`slide_rect`]); this module stays syscall-side. Auto-hide never reserves:
//! the strip overlays other windows, which keep their full space hidden or
//! revealed (ticket 63, taskbar parity). `fixed` mode keeps the strip
//! permanently reserved via [`reserve`]; maximized windows on the docked edge
//! shrink to accommodate it (ADR-0011).
//!
//! Ticket 60: the auto-hide machinery is made honest. The AppBar registers a
//! real callback message ([`callback_message`]) in `uCallbackMessage` so the
//! shell's `ABN_*` notifications reach the window (the consumer in
//! `quick_window` subclasses the window to receive them). Auto-hide is applied
//! per edge — never a hardcoded `ABE_LEFT` — and every enable is verified with
//! `ABM_GETAUTOHIDEBAR`, so a refused engagement (another auto-hide bar owns
//! the edge) is surfaced instead of silently no-op'ing.
//!
//! Ticket 61: the documented AppBar pattern is followed exactly. `ABM_QUERYPOS`
//! adjusts the rect by subtraction and does not preserve the strip thickness,
//! so it is re-applied ([`apply_thickness`]) before `ABM_SETPOS`; the window
//! is placed at the rect `ABM_SETPOS` returns ([`place`]), never a rect of our
//! own — the divergence between the reserved rect and the placed window was
//! the overlap root cause. Every `SHAppBarMessage` result is logged on
//! failure. The drift helpers ([`window_rect`], [`rects_diverged`],
//! [`mostly_overlapping`]) back the Quick Launch window's periodic re-assert.
//!
//! The syscall surface (`SHAppBarMessage`) is not unit-testable on CI; the
//! geometry math is factored into pure functions tested below.

use std::mem::size_of;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::Shell::{
    SHAppBarMessage, ABE_LEFT, ABE_RIGHT, ABM_GETAUTOHIDEBAR, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE,
    ABM_SETAUTOHIDEBAR, ABM_SETPOS, APPBARDATA,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetWindowRect, RegisterWindowMessageW, SetWindowPos, SWP_NOACTIVATE, SWP_NOSENDCHANGING,
    SWP_NOZORDER, HWND_TOPMOST,
};

/// The registered AppBar callback message (ticket 60). `RegisterWindowMessage`
/// hands out a unique id in the 0xC000–0xFFFF range, registered once per
/// process; the shell sends it to the bar's window with the `ABN_*`
/// notification code in `wparam`. Without it, `APPBARDATA.uCallbackMessage` is
/// 0 and the window never learns about auto-hide state changes.
pub fn callback_message() -> u32 {
    static MSG: OnceLock<u32> = OnceLock::new();
    *MSG.get_or_init(|| unsafe {
        let wide: Vec<u16> = "SproutAppBarCallbackMsg"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        RegisterWindowMessageW(wide.as_ptr())
    })
}

/// The screen-edge constant for a settings edge string ("left" / "right").
pub fn edge_constant(edge: &str) -> Option<u32> {
    match edge {
        "left" => Some(ABE_LEFT),
        "right" => Some(ABE_RIGHT),
        _ => None,
    }
}

/// The AppBar's desired rectangle: a full-height strip of `width` physical
/// pixels against the given work area's left or right edge.
pub fn appbar_rect(work: RECT, edge: u32, width: i32) -> RECT {
    match edge {
        ABE_LEFT => RECT {
            left: work.left,
            top: work.top,
            right: work.left + width,
            bottom: work.bottom,
        },
        ABE_RIGHT => RECT {
            left: work.right - width,
            top: work.top,
            right: work.right,
            bottom: work.bottom,
        },
        _ => work,
    }
}

/// The full info for the monitor `hwnd` currently sits on — the one
/// `MonitorFromWindow`→`GetMonitorInfoW` probe; the accessors below return
/// different fields of it.
fn monitor_info(hwnd: HWND) -> Option<MONITORINFOEXW> {
    unsafe {
        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }
        let mut info: MONITORINFOEXW = std::mem::zeroed();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.monitorInfo as *mut _) == 0 {
            return None;
        }
        Some(info)
    }
}

/// The monitor's work area (the screen area minus the taskbar) for the
/// monitor the window currently sits on.
pub fn work_area(hwnd: HWND) -> Option<RECT> {
    monitor_info(hwnd).map(|info| info.monitorInfo.rcWork)
}

/// The monitor's full rectangle (`rcMonitor` — reservations and taskbar
/// included): the geometry base for the auto-hide overlay strip (ticket 63),
/// which spans the screen edge whether or not anything is reserved there.
pub fn monitor_rect(hwnd: HWND) -> Option<RECT> {
    monitor_info(hwnd).map(|info| info.monitorInfo.rcMonitor)
}

/// A stable per-monitor key for the dock's memory: the monitor's device name
/// (e.g. `\\.\DISPLAY1`), which survives display rearrangements and reboots.
pub fn monitor_key(hwnd: HWND) -> Option<String> {
    let name = String::from_utf16_lossy(&monitor_info(hwnd)?.szDevice);
    let name = name.trim_end_matches('\0').to_string();
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}

/// The rect to re-assert a docked bar at (ticket 61): the shell recomputes
/// the work area from the registered bars, so a flush bar's own placement
/// makes the work area's edge sit exactly at the bar's inner edge (rcWork
/// already reflects the bar's own reservation). In that state the bar is
/// where the shell expects it — keep its horizontal position and only follow
/// the work area's top/bottom (a taskbar moved). Any other edge means the
/// work area genuinely changed against the bar (an intruder took or freed the
/// edge) — re-align it to the new edge. Re-deriving the docked side from the
/// self-shrunk work area is what marched the bar one width into the screen
/// on every notification ("dock to the right shifts everything left").
pub fn desired_rect(edge: u32, work: RECT, current: RECT, width: i32) -> RECT {
    match edge {
        ABE_RIGHT => {
            if (work.right - current.left).abs() <= 2 {
                RECT {
                    left: current.left,
                    top: work.top,
                    right: current.right,
                    bottom: work.bottom,
                }
            } else {
                appbar_rect(work, edge, width)
            }
        }
        ABE_LEFT => {
            if (work.left - current.right).abs() <= 2 {
                RECT {
                    left: current.left,
                    top: work.top,
                    right: current.right,
                    bottom: work.bottom,
                }
            } else {
                appbar_rect(work, edge, width)
            }
        }
        _ => current,
    }
}

/// Registers the window as an AppBar at `edge` (`ABM_NEW`) and returns the
/// final rect the window must be placed in — the rect `ABM_SETPOS` returns
/// (ticket 61), which is the system's own answer to where the bar lives. The
/// auto-hide mode is applied separately afterwards ([`set_autohide`]) once the
/// bar is registered and placed, so a refused engagement keeps the dock
/// instead of unwinding the registration. A partially succeeded registration
/// (e.g. `ABM_SETPOS` refused) is an `Err` — the caller releases the AppBar so
/// no half-docked bar is left behind.
pub fn register(hwnd: HWND, edge: u32) -> Result<(), String> {
    unsafe {
        let mut data = appbar_data(hwnd, edge, empty_rect());
        let result = SHAppBarMessage(ABM_NEW, &mut data);
        if result == 0 {
            eprintln!("ABM_NEW refused (result {result}) for hwnd {hwnd:?}");
            return Err("the system rejected the app bar registration".into());
        }
        Ok(())
    }
}

/// Reserves workspace space for the bar (ticket 63): re-queries the rect,
/// re-applies the strip thickness, commits it with `ABM_SETPOS`, and returns
/// the granted rect — the caller places the window at it. Only `fixed` mode
/// reserves; auto-hide registers without ever calling this (other windows
/// keep their full space). Also serves edge switches and shell-initiated
/// changes (`ABN_POSCHANGED`); the auto-hide mode is applied by the caller
/// after the reservation is in place.
pub fn reserve(hwnd: HWND, edge: u32, desired: RECT) -> Result<RECT, String> {
    unsafe {
        let mut data = appbar_data(hwnd, edge, desired);
        let result = SHAppBarMessage(ABM_QUERYPOS, &mut data);
        if result == 0 {
            eprintln!("ABM_QUERYPOS refused (result {result}) after a successful ABM_NEW");
            return Err("the system rejected the app bar position query".into());
        }
        // ABM_QUERYPOS adjusts the rect by subtracting the bar from the work
        // area and does not preserve the strip thickness — re-apply it before
        // ABM_SETPOS, or the bar would collapse to the leftover edge space.
        data.rc = apply_thickness(data.rc, edge, width_of(desired));
        let mut set = appbar_data(hwnd, edge, data.rc);
        let result = SHAppBarMessage(ABM_SETPOS, &mut set);
        if result == 0 {
            eprintln!("ABM_SETPOS refused (result {result}) after a successful ABM_NEW");
            return Err("the system rejected the app bar placement".into());
        }
        // The window is placed at the rect ABM_SETPOS returned — the reserved
        // rect and the placed window are the same rect, which is what keeps
        // the bar from overlapping other windows on any desktop.
        Ok(set.rc)
    }
}

/// Re-applies the strip thickness to the rect `ABM_QUERYPOS` returned (ticket
/// 61): the query adjusts the rect by subtracting the bar from the work area
/// and does not preserve thickness, so it is re-applied from the edge before
/// `ABM_SETPOS`.
pub fn apply_thickness(rect: RECT, edge: u32, width: i32) -> RECT {
    match edge {
        ABE_LEFT => RECT {
            right: rect.left + width,
            ..rect
        },
        ABE_RIGHT => RECT {
            left: rect.right - width,
            ..rect
        },
        _ => rect,
    }
}

/// Places the window at `rect` — position and size in one `SetWindowPos` call
/// (ticket 61), so re-docks and edge switches apply atomically with no
/// hide/show and no flicker. The z-order is untouched (`SWP_NOZORDER`) unless
/// `topmost` — then the window is raised into the topmost band, so apps
/// opened (or restored/maximized) afterwards never cover the strip (ticket
/// 66 follow-up: the shell's promised auto-hide z-order maintenance does not
/// survive our Sprout-driven motion; the driver re-asserts it on every
/// placement). Fixed-mode bars keep the non-topmost taskbar behavior.
pub fn place(hwnd: HWND, rect: RECT, topmost: bool) -> Result<(), String> {
    unsafe {
        let insert_after = if topmost { HWND_TOPMOST } else { std::ptr::null_mut() };
        let mut flags = SWP_NOACTIVATE | SWP_NOSENDCHANGING;
        if !topmost {
            flags |= SWP_NOZORDER;
        }
        let ok = SetWindowPos(
            hwnd,
            insert_after,
            rect.left,
            rect.top,
            rect.right - rect.left,
            rect.bottom - rect.top,
            flags,
        );
        if ok == 0 {
            return Err("could not place the app bar window".into());
        }
    }
    Ok(())
}

/// The window's actual on-screen rectangle (`GetWindowRect`) — the drift
/// check's ground truth (ticket 61).
pub fn window_rect(hwnd: HWND) -> Option<RECT> {
    unsafe {
        let mut rect: RECT = std::mem::zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return None;
        }
        Some(rect)
    }
}

/// Whether `actual` and `expected` differ by more than `tolerance` pixels on
/// any side — the drift check's divergence test (ticket 61).
pub fn rects_diverged(actual: RECT, expected: RECT, tolerance: i32) -> bool {
    (actual.left - expected.left).abs() > tolerance
        || (actual.top - expected.top).abs() > tolerance
        || (actual.right - expected.right).abs() > tolerance
        || (actual.bottom - expected.bottom).abs() > tolerance
}

/// The area of the intersection of two rects (zero when they do not overlap).
fn intersection_area(a: RECT, b: RECT) -> i64 {
    let width = a.right.min(b.right) - a.left.max(b.left);
    let height = a.bottom.min(b.bottom) - a.top.max(b.top);
    if width <= 0 || height <= 0 {
        0
    } else {
        width as i64 * height as i64
    }
}

/// Whether `actual` covers at least `min_fraction` of `expected`'s area — the
/// auto-hide guard for the drift check (ticket 61): a hidden bar's sliver
/// overlaps a tiny fraction of the strip rect, a revealed or drifted bar
/// overlaps most of it, so the check never fights the OS's slide while
/// auto-hidden.
pub fn mostly_overlapping(actual: RECT, expected: RECT, min_fraction: f64) -> bool {
    let expected_area =
        (expected.right - expected.left) as i64 * (expected.bottom - expected.top) as i64;
    if expected_area <= 0 {
        return false;
    }
    intersection_area(actual, expected) as f64 / expected_area as f64 >= min_fraction
}

/// The auto-hide sliver for a docked strip (ticket 63): the strip collapsed
/// to `sliver` physical pixels against its docked edge — the only part left
/// on-screen while hidden. Top/bottom (the strip's height) are unchanged.
pub fn sliver_rect(strip: RECT, edge: u32, sliver: i32) -> RECT {
    match edge {
        ABE_LEFT => RECT {
            right: strip.left + sliver,
            ..strip
        },
        ABE_RIGHT => RECT {
            left: strip.right - sliver,
            ..strip
        },
        _ => strip,
    }
}

/// Whether the cursor (`x`, `y`) is inside the edge trigger band (ticket 63):
/// within `trigger` physical pixels of the strip's docked edge and inside its
/// vertical extent — touching the screen edge reveals a hidden strip. The
/// vertical bound keeps a cursor on an adjacent monitor (or in the corner
/// past the strip) from triggering.
pub fn edge_hit(x: i32, y: i32, strip: RECT, edge: u32, trigger: i32) -> bool {
    let in_band = match edge {
        ABE_LEFT => x <= strip.left + trigger,
        ABE_RIGHT => x >= strip.right - trigger,
        _ => false,
    };
    let in_height = y >= strip.top && y <= strip.bottom;
    in_band && in_height
}

/// Whether the cursor (`x`, `y`) is inside the strip's bounding rectangle
/// (ticket 63): hovering anywhere over the strip — including its sliver —
/// keeps it revealed; leaving it lets it hide again.
pub fn strip_contains(x: i32, y: i32, strip: RECT) -> bool {
    x >= strip.left && x <= strip.right && y >= strip.top && y <= strip.bottom
}

/// Ease-out cubic (ticket 63): fast start, gentle settle — the slide's feel.
/// `t` is clamped to 0..1; the output spans the same range with
/// `ease(0.5) = 0.875`.
pub fn ease_out_cubic(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// One animation frame between two rects (ticket 63): each side interpolates
/// from `from` toward `to` by the eased progress `t` (0 = fully `from`,
/// 1 = fully `to`), rounded to whole physical pixels. Pure — the driver calls
/// it every tick with the elapsed fraction of the slide duration.
pub fn slide_rect(from: RECT, to: RECT, t: f64) -> RECT {
    let k = ease_out_cubic(t);
    let lerp = |a: i32, b: i32| (a as f64 + (b - a) as f64 * k).round() as i32;
    RECT {
        left: lerp(from.left, to.left),
        top: lerp(from.top, to.top),
        right: lerp(from.right, to.right),
        bottom: lerp(from.bottom, to.bottom),
    }
}

/// Unregisters the window as an AppBar — the edge is released and other
/// windows get their full space back. Safe to call on a non-appbar window.
pub fn remove(hwnd: HWND) {
    unsafe {
        let mut data = appbar_data(hwnd, ABE_LEFT, empty_rect());
        SHAppBarMessage(ABM_REMOVE, &mut data);
    }
}

/// Whether the shell currently registers `hwnd` as the auto-hide bar on
/// `edge` (`ABM_GETAUTOHIDEBAR`). This is the OS's own answer to "is auto-hide
/// really on for me here" — the source of truth the docked mode reconciles
/// against.
pub fn autohide_engaged(hwnd: HWND, edge: u32) -> bool {
    unsafe {
        let mut data = appbar_data(hwnd, edge, empty_rect());
        SHAppBarMessage(ABM_GETAUTOHIDEBAR, &mut data) as isize == hwnd as isize
    }
}

/// Enables or disables the auto-hide *registration* at `edge` (ticket 63).
/// Registration grants coordination only — exclusivity, z-order, work-area
/// semantics — never motion; the slide itself is Sprout's driver in
/// `quick_window`. The engagement is verified with `ABM_GETAUTOHIDEBAR`
/// afterwards: a refused enable (another auto-hide bar already owns the
/// edge — e.g. the taskbar's own auto-hide there) is an `Err` whose message
/// is the honest reason surfaced in the Quick Launch window, so the caller
/// never assumes the request took; a refused/absent disable is a harmless
/// `Ok(false)`. Hiding works regardless of this outcome.
pub fn set_autohide(hwnd: HWND, edge: u32, enabled: bool) -> Result<bool, String> {
    unsafe {
        let mut data = appbar_data(hwnd, edge, empty_rect());
        data.lParam = if enabled { 1 } else { 0 };
        SHAppBarMessage(ABM_SETAUTOHIDEBAR, &mut data);
        let engaged = autohide_engaged(hwnd, edge);
        if enabled && !engaged {
            return Err("another auto-hide bar already owns this edge — the system refused auto-hide"
                .into());
        }
        Ok(engaged)
    }
}

fn empty_rect() -> RECT {
    RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    }
}

/// The rect's width — the thickness re-applied after `ABM_QUERYPOS` (ticket
/// 61): the caller's desired strip width, not whatever the query left.
fn width_of(rect: RECT) -> i32 {
    rect.right - rect.left
}

fn appbar_data(hwnd: HWND, edge: u32, rc: RECT) -> APPBARDATA {
    APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: callback_message(),
        uEdge: edge,
        rc,
        lParam: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::window::{DOCK_WIDTH, WINDOW_WIDTH};

    #[test]
    fn left_edge_strips_the_work_area() {
        let work = RECT {
            left: 0,
            top: 40,
            right: 1920,
            bottom: 1040,
        };
        let rect = appbar_rect(work, ABE_LEFT, DOCK_WIDTH as i32);
        assert_eq!(rect.left, 0);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.right, DOCK_WIDTH as i32);
        assert_eq!(rect.bottom, 1040);
    }

    #[test]
    fn right_edge_strips_the_work_area() {
        let work = RECT {
            left: 0,
            top: 40,
            right: 1920,
            bottom: 1040,
        };
        let rect = appbar_rect(work, ABE_RIGHT, DOCK_WIDTH as i32);
        assert_eq!(rect.left, 1920 - DOCK_WIDTH as i32);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.right, 1920);
        assert_eq!(rect.bottom, 1040);
    }

    #[test]
    fn dock_width_matches_the_floating_window_width() {
        // Ticket 56: the docked strip is exactly as wide as the floating
        // window, so dock → undock never changes the window's width.
        assert_eq!(DOCK_WIDTH, WINDOW_WIDTH);
    }

    #[test]
    fn edge_constant_maps_settings_strings() {
        assert_eq!(edge_constant("left"), Some(ABE_LEFT));
        assert_eq!(edge_constant("right"), Some(ABE_RIGHT));
        assert_eq!(edge_constant("top"), None);
    }

    #[test]
    fn callback_message_is_a_registered_message_id() {
        // Registered messages live in the 0xC000–0xFFFF range — a real,
        // process-unique id, never 0 (ticket 60: uCallbackMessage was 0 before,
        // which is why no ABN_* notification ever arrived).
        let msg = callback_message();
        assert!((0xC000..=0xFFFF).contains(&msg));
        assert_eq!(callback_message(), msg);
    }

    #[test]
    fn querypos_rect_gets_thickness_reapplied_on_the_left() {
        // ABM_QUERYPOS adjusts by subtraction and does not preserve thickness
        // (ticket 61): a left-edge query returns a zero-width rect at the work
        // area's left edge — the thickness is re-applied from the edge.
        let queried = RECT {
            left: 0,
            top: 40,
            right: 0,
            bottom: 1040,
        };
        let rect = apply_thickness(queried, ABE_LEFT, DOCK_WIDTH as i32);
        assert_eq!(rect.left, 0);
        assert_eq!(rect.right, DOCK_WIDTH as i32);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.bottom, 1040);
    }

    #[test]
    fn querypos_rect_gets_thickness_reapplied_on_the_right() {
        let queried = RECT {
            left: 1920,
            top: 40,
            right: 1920,
            bottom: 1040,
        };
        let rect = apply_thickness(queried, ABE_RIGHT, DOCK_WIDTH as i32);
        assert_eq!(rect.left, 1920 - DOCK_WIDTH as i32);
        assert_eq!(rect.right, 1920);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.bottom, 1040);
    }

    #[test]
    fn setpos_rect_keeps_the_reapplied_thickness() {
        // The rect the window is placed at (the ABM_SETPOS-returned rect,
        // ticket 61) is the queried rect with the thickness re-applied — the
        // reserved rect and the placed window are one rect, never two.
        let queried = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 1000,
        };
        let rect = apply_thickness(queried, ABE_LEFT, 340);
        assert_eq!(rect.right - rect.left, 340);
        assert_eq!(rect.bottom - rect.top, 1000);
    }

    #[test]
    fn drift_divergence_and_overlap_checks() {
        let expected = RECT {
            left: 0,
            top: 0,
            right: 340,
            bottom: 1000,
        };
        assert!(!rects_diverged(expected, expected, 2));
        let moved = RECT {
            left: 100,
            top: 0,
            right: 440,
            bottom: 1000,
        };
        assert!(rects_diverged(moved, expected, 2));
        // A fully covered rect is "mostly overlapping"…
        assert!(mostly_overlapping(expected, expected, 0.5));
        // …a hidden auto-hide bar's sliver is not…
        let sliver = RECT {
            left: -338,
            top: 0,
            right: 2,
            bottom: 1000,
        };
        assert!(!mostly_overlapping(sliver, expected, 0.5));
        // …and neither is a bar moved to another monitor.
        let elsewhere = RECT {
            left: 2000,
            top: 0,
            right: 2340,
            bottom: 1000,
        };
        assert!(!mostly_overlapping(elsewhere, expected, 0.5));
        assert_eq!(intersection_area(moved, expected), 240 * 1000);
        assert_eq!(intersection_area(elsewhere, expected), 0);
    }

    #[test]
    fn desired_rect_keeps_a_bar_that_matches_its_own_reservation() {
        // The shell recomputes the work area from the registered bars, so a
        // right-docked bar's own placement makes rcWork.right sit exactly at
        // the bar's inner edge. Re-asserting from that self-shrunk work area
        // would grant a rect one width into the screen — the "shifts
        // everything left" march (ticket 61). The current rect is kept.
        let work = RECT { left: 0, top: 0, right: 2220, bottom: 1848 };
        let current = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let desired = desired_rect(ABE_RIGHT, work, current, 340);
        assert_eq!(desired.left, 2220);
        assert_eq!(desired.right, 2560);
        // The left edge mirrors it: rcWork.left sits at the bar's inner edge.
        let work = RECT { left: 340, top: 0, right: 2560, bottom: 1848 };
        let current = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let desired = desired_rect(ABE_LEFT, work, current, 340);
        assert_eq!(desired.left, 0);
        assert_eq!(desired.right, 340);
    }

    #[test]
    fn desired_rect_keeps_a_sub_pixel_rounding_gap() {
        // A one-pixel shell rounding gap is still "unchanged" — never step a
        // flush bar into the screen over a rounding error.
        let work = RECT { left: 0, top: 0, right: 2219, bottom: 1848 };
        let current = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let desired = desired_rect(ABE_RIGHT, work, current, 340);
        assert_eq!(desired.left, 2220);
    }

    #[test]
    fn desired_rect_reasserts_against_a_genuinely_changed_work_area() {
        // The work area's edge moved INSIDE the bar's strip (another bar or a
        // taskbar took the edge): the bar must move over against the new edge.
        let work = RECT { left: 0, top: 0, right: 2480, bottom: 1848 };
        let current = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let desired = desired_rect(ABE_RIGHT, work, current, 340);
        assert_eq!(desired.left, 2140);
        assert_eq!(desired.right, 2480);
        // The work area's edge moved OUTSIDE the strip (the intruder left):
        // the bar returns to the freed edge.
        let work = RECT { left: 0, top: 0, right: 2560, bottom: 1848 };
        let current = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let desired = desired_rect(ABE_RIGHT, work, current, 340);
        assert_eq!(desired.left, 2220);
        assert_eq!(desired.right, 2560);
    }

    #[test]
    fn desired_rect_reasserts_when_the_strip_height_changes() {
        // A bottom taskbar appeared or resized: the full-height strip must
        // shrink/grow with the work area while the docked edge itself stays
        // where the bar is — the work area's edge still reflects only the
        // bar's own reservation.
        let work = RECT { left: 0, top: 0, right: 2220, bottom: 1700 };
        let current = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let desired = desired_rect(ABE_RIGHT, work, current, 340);
        assert_eq!(desired.top, 0);
        assert_eq!(desired.bottom, 1700);
        assert_eq!(desired.left, 2220);
        assert_eq!(desired.right, 2560);
        // Same for the left edge with a top taskbar.
        let work = RECT { left: 340, top: 40, right: 2560, bottom: 1848 };
        let current = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let desired = desired_rect(ABE_LEFT, work, current, 340);
        assert_eq!(desired.top, 40);
        assert_eq!(desired.bottom, 1848);
        assert_eq!(desired.left, 0);
        assert_eq!(desired.right, 340);
    }

    #[test]
    fn sliver_collapses_the_strip_against_its_edge() {
        // Ticket 63: the hidden strip keeps only a 2 px handle against its
        // docked edge; height and the outer edge stay put.
        let strip = RECT { left: 0, top: 40, right: 340, bottom: 1040 };
        let sliver = sliver_rect(strip, ABE_LEFT, 2);
        assert_eq!(sliver.left, 0);
        assert_eq!(sliver.right, 2);
        assert_eq!(sliver.top, 40);
        assert_eq!(sliver.bottom, 1040);
        let strip = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let sliver = sliver_rect(strip, ABE_RIGHT, 2);
        assert_eq!(sliver.left, 2558);
        assert_eq!(sliver.right, 2560);
    }

    #[test]
    fn edge_hit_requires_the_band_and_the_strip_height() {
        // Ticket 63: within EDGE_TRIGGER_PX of the docked edge and inside the
        // strip's vertical extent reveals; anywhere else does not.
        let strip = RECT { left: 0, top: 40, right: 340, bottom: 1040 };
        assert!(edge_hit(0, 500, strip, ABE_LEFT, 8));
        assert!(edge_hit(8, 40, strip, ABE_LEFT, 8)); // band edge inclusive
        assert!(!edge_hit(9, 500, strip, ABE_LEFT, 8));
        assert!(!edge_hit(4, 39, strip, ABE_LEFT, 8)); // above the strip
        assert!(!edge_hit(4, 1041, strip, ABE_LEFT, 8)); // below the strip
        let right = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        assert!(edge_hit(2560, 900, right, ABE_RIGHT, 8));
        assert!(edge_hit(2552, 900, right, ABE_RIGHT, 8));
        assert!(!edge_hit(2551, 900, right, ABE_RIGHT, 8));
    }

    #[test]
    fn strip_contains_is_inclusive_and_sliver_friendly() {
        // Ticket 63: hovering anywhere over the strip (or its sliver) counts;
        // the boundary itself is inside so the bar never stutters at 1 px out.
        let strip = RECT { left: 0, top: 40, right: 340, bottom: 1040 };
        assert!(strip_contains(0, 40, strip));
        assert!(strip_contains(340, 1040, strip));
        assert!(strip_contains(200, 500, strip));
        let sliver = sliver_rect(strip, ABE_LEFT, 2);
        assert!(strip_contains(1, 500, sliver));
        assert!(!strip_contains(341, 500, strip));
        assert!(!strip_contains(200, 39, strip));
    }

    #[test]
    fn ease_out_cubic_spans_and_bulges() {
        // Ticket 63: endpoints exact, monotone, fast start (half the time,
        // most of the way).
        assert_eq!(ease_out_cubic(0.0), 0.0);
        assert_eq!(ease_out_cubic(1.0), 1.0);
        assert_eq!(ease_out_cubic(0.5), 0.875);
        assert_eq!(ease_out_cubic(-1.0), 0.0); // clamped
        assert_eq!(ease_out_cubic(2.0), 1.0); // clamped
        let mut prev = 0.0;
        for step in 1..=10 {
            let t = ease_out_cubic(step as f64 / 10.0);
            assert!(t > prev);
            prev = t;
        }
    }

    #[test]
    fn slide_rect_interpolates_whole_pixels_with_easing() {
        // Ticket 63: t=0 is `from`, t=1 is exactly `to`; the eased midpoint
        // sits 87.5% of the way — rounded to whole pixels.
        let from = RECT { left: 340, top: 40, right: 680, bottom: 1040 }; // slid away (left dock)
        let to = RECT { left: 0, top: 40, right: 340, bottom: 1040 };
        let start = slide_rect(from, to, 0.0);
        assert_eq!((start.left, start.top, start.right, start.bottom), (340, 40, 680, 1040));
        let end = slide_rect(from, to, 1.0);
        assert_eq!((end.left, end.top, end.right, end.bottom), (0, 40, 340, 1040));
        let mid = slide_rect(from, to, 0.5);
        assert_eq!(mid.left, 43); // 340 - 340*0.875 = 42.5 → rounds to 43
        assert_eq!(mid.right, 383);
        assert_eq!(mid.top, 40); // unaffected axis stays exact
        // The hide direction mirrors it.
        let back = slide_rect(to, from, 0.5);
        assert_eq!(back.left, 298); // 340*0.875 = 297.5 → rounds away from zero
    }
}