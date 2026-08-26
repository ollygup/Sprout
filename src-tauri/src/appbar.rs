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

use windows_sys::Win32::Devices::Display::{
    DisplayConfigGetDeviceInfo, GetDisplayConfigBufferSizes, QueryDisplayConfig,
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
};
use windows_sys::Win32::Foundation::{HWND, LUID, RECT};
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

/// The dock memory identity for the monitor `hwnd` sits on (ticket 110): the
/// panel's EDID make+product code, which follows the physical display across
/// replugs and slot renumbering where the device-name key does not. `None`
/// means no usable identity — virtual/remote displays with empty EDID data,
/// or a shell query failure — and the caller falls back to [`monitor_key`].
pub fn monitor_identity(hwnd: HWND) -> Option<String> {
    let device = {
        let info = monitor_info(hwnd)?;
        let name = String::from_utf16_lossy(&info.szDevice);
        let name = name.trim_end_matches('\0').to_string();
        if name.is_empty() {
            return None;
        }
        name
    };
    display_identity(&device)
}

/// The EDID make+product of the active display-config path whose source GDI
/// device name is `device`. Syscall-side by module convention; the pieces it
/// composes ([`edid_identity`], [`wide_matches`]) are pure and tested below.
fn display_identity(device: &str) -> Option<String> {
    unsafe {
        let mut num_paths = 0u32;
        let mut num_modes = 0u32;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut num_paths, &mut num_modes) != 0 {
            return None;
        }
        if num_paths == 0 {
            return None;
        }
        let mut paths: Vec<DISPLAYCONFIG_PATH_INFO> = vec![std::mem::zeroed(); num_paths as usize];
        let mut modes: Vec<DISPLAYCONFIG_MODE_INFO> = vec![std::mem::zeroed(); num_modes as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut num_paths,
            paths.as_mut_ptr(),
            &mut num_modes,
            modes.as_mut_ptr(),
            std::ptr::null_mut(),
        ) != 0
        {
            return None;
        }
        paths.truncate(num_paths as usize);
        for path in &paths {
            let mut source: DISPLAYCONFIG_SOURCE_DEVICE_NAME = std::mem::zeroed();
            source.header = device_info_header(
                DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                path.sourceInfo.adapterId,
                path.sourceInfo.id,
                size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            );
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            if !wide_matches(&source.viewGdiDeviceName, device) {
                continue;
            }
            let mut target: DISPLAYCONFIG_TARGET_DEVICE_NAME = std::mem::zeroed();
            target.header = device_info_header(
                DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
                path.targetInfo.adapterId,
                path.targetInfo.id,
                size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            );
            if DisplayConfigGetDeviceInfo(&mut target.header) != 0 {
                continue;
            }
            // An all-zero EDID pair carries no identity — two different
            // virtual displays would collide on one key — so treat it as
            // "no usable identity" rather than a shared bucket.
            if target.edidManufactureId == 0 && target.edidProductCodeId == 0 {
                return None;
            }
            return Some(edid_identity(
                target.edidManufactureId.into(),
                target.edidProductCodeId.into(),
            ));
        }
        None
    }
}

/// The request header [`DisplayConfigGetDeviceInfo`] keys on.
fn device_info_header(
    kind: i32,
    adapter_id: LUID,
    id: u32,
    size: u32,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size,
        adapterId: adapter_id,
        id,
    }
}

/// The storage-suffix form of an EDID make+product pair: deterministic hex so
/// the same physical panel always yields the same string.
fn edid_identity(manufacture_id: u32, product_code: u32) -> String {
    format!("edid-{manufacture_id:04X}-{product_code:04X}")
}

/// Whether a NUL-terminated wide string equals `expected`, case-insensitively
/// (GDI device names compare without case in practice).
fn wide_matches(raw: &[u16], expected: &str) -> bool {
    let len = raw.iter().position(|&c| c == 0).unwrap_or(raw.len());
    let text = String::from_utf16_lossy(&raw[..len]);
    text.eq_ignore_ascii_case(expected)
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

/// Whether the cursor (`x`, `y`) is inside the edge trigger band (ticket 112):
/// within [`AUTOHIDE_SLIVER_PX`] of the strip's docked edge and inside its
/// vertical extent — the reserved invisible zone itself, not an interior band
/// (single size source). The vertical bound keeps a cursor on an adjacent
/// monitor from triggering.
#[allow(dead_code)]
pub fn edge_hit(x: i32, y: i32, strip: RECT, edge: u32) -> bool {
    in_reveal_band(x, y, strip, edge)
}

/// Whether the cursor is inside the reveal band (ticket 112): the sliver
/// itself — the only trigger zone. Within [`AUTOHIDE_SLIVER_PX`] of the
/// docked edge and inside the strip's vertical extent.
pub fn in_reveal_band(x: i32, y: i32, strip: RECT, edge: u32) -> bool {
    use crate::constants::window::AUTOHIDE_SLIVER_PX;
    let band = sliver_rect(strip, edge, AUTOHIDE_SLIVER_PX);
    x >= band.left && x <= band.right && y >= band.top && y <= band.bottom
}

/// The reveal gate's poll-loop memory (ticket 112): accumulated toward-edge
/// travel, dwell timer start, and previous cursor position. Pure — the driver
/// threads it through each tick, tests drive it with injected timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RevealGate {
    pub accumulated: i32,
    pub dwell_start_ms: Option<u64>,
    pub prev: Option<(i32, i32)>,
}

/// One reveal-gate step (ticket 112): pure decision logic behind the layered
/// gate. Returns the updated gate and whether the dock should reveal.
///
/// Layers:
/// 1. Trigger zone — cursor must be inside the sliver band itself.
/// 2. Direction gating — samples dominated by along-edge motion (dy > dx_toward
///    or moving away) accumulate nothing; toward-edge travel accumulates,
///    capped per sample to [`REVEAL_MAX_STEP_PX`], until
///    `sensitivity` is reached.
/// 3. Dwell — once sensitivity is reached, the cursor must stay inside the band
///    for `dwell_ms` (any exit cancels instantly and resets accumulation).
///
/// Cross-monitor transit through the band never reveals because it exits
/// within a frame or two, aborting the dwell before it elapses — no per-topology
/// special case is needed.
pub fn reveal_gate_step(
    mut state: RevealGate,
    x: i32,
    y: i32,
    now_ms: u64,
    full: RECT,
    edge: u32,
    sensitivity: i32,
    dwell_ms: u64,
) -> (RevealGate, bool) {
    use crate::constants::window::REVEAL_MAX_STEP_PX;
    if !in_reveal_band(x, y, full, edge) {
        state.accumulated = 0;
        state.dwell_start_ms = None;
        state.prev = Some((x, y));
        return (state, false);
    }
    if let Some((px, py)) = state.prev {
        let toward = match edge {
            ABE_LEFT => px - x,
            ABE_RIGHT => x - px,
            _ => 0,
        };
        let dy = (y - py).abs();
        let dominated = toward <= 0 || dy > toward;
        if !dominated {
            let inc = toward.min(REVEAL_MAX_STEP_PX).max(0);
            state.accumulated = (state.accumulated + inc).min(sensitivity + REVEAL_MAX_STEP_PX);
        }
        if state.accumulated >= sensitivity {
            if state.dwell_start_ms.is_none() {
                state.dwell_start_ms = Some(now_ms);
            }
            if let Some(start) = state.dwell_start_ms {
                if now_ms.saturating_sub(start) >= dwell_ms {
                    state.prev = Some((x, y));
                    return (state, true);
                }
            }
        }
    }
    state.prev = Some((x, y));
    (state, false)
}

/// Convenience wrapper using the shipped constants (ticket 112).
#[allow(dead_code)]
pub fn reveal_gate_step_default(
    state: RevealGate,
    x: i32,
    y: i32,
    now_ms: u64,
    full: RECT,
    edge: u32,
) -> (RevealGate, bool) {
    use crate::constants::window::{REVEAL_DWELL_MS, REVEAL_SENSITIVITY_PX};
    reveal_gate_step(state, x, y, now_ms, full, edge, REVEAL_SENSITIVITY_PX, REVEAL_DWELL_MS)
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
        // Ticket 112: trigger zone is the sliver itself (single size source) — within
        // AUTOHIDE_SLIVER_PX of the docked edge and inside vertical extent.
        let strip = RECT { left: 0, top: 40, right: 340, bottom: 1040 };
        assert!(edge_hit(0, 500, strip, ABE_LEFT));
        assert!(edge_hit(2, 40, strip, ABE_LEFT)); // sliver edge inclusive
        assert!(!edge_hit(3, 500, strip, ABE_LEFT));
        assert!(!edge_hit(1, 39, strip, ABE_LEFT)); // above the strip
        assert!(!edge_hit(1, 1041, strip, ABE_LEFT)); // below the strip
        let right = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        assert!(edge_hit(2560, 900, right, ABE_RIGHT));
        assert!(edge_hit(2558, 900, right, ABE_RIGHT));
        assert!(!edge_hit(2557, 900, right, ABE_RIGHT));
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

    #[test]
    fn edid_identity_is_deterministic_hex() {
        // Ticket 110: the storage suffix is stable across calls and machines —
        // fixed-width uppercase hex so the same panel always yields the same
        // string.
        assert_eq!(edid_identity(0x1234, 0x5678), "edid-1234-5678");
        assert_eq!(edid_identity(0xA, 0xB), "edid-000A-000B");
        assert_eq!(edid_identity(1, 2), edid_identity(1, 2));
        assert_ne!(edid_identity(1, 2), edid_identity(2, 1));
    }

    #[test]
    fn wide_matches_compares_nul_terminated_wide_strings_without_case() {
        let mut raw: Vec<u16> = r"\\.\DISPLAY1".encode_utf16().collect();
        raw.push(0);
        assert!(wide_matches(&raw, r"\\.\DISPLAY1"));
        // Windows device names compare without case in practice.
        assert!(wide_matches(&raw, r"\\.\display1"));
        assert!(!wide_matches(&raw, r"\\.\DISPLAY2"));
        // A buffer with no terminator compares up to its full length.
        let unterminated: Vec<u16> = "DISPLAY".encode_utf16().collect();
        assert!(wide_matches(&unterminated, "display"));
    }

    #[test]
    fn reveal_gate_graze_along_the_seam_never_reveals() {
        // Ticket 112: graze — cursor slides along the seam inside the sliver band
        // with dominant along-edge motion (dy >> dx_toward) accumulates nothing.
        let full = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let mut gate = RevealGate::default();
        let sensitivity = crate::constants::window::REVEAL_SENSITIVITY_PX;
        let dwell = crate::constants::window::REVEAL_DWELL_MS;
        // Enter the sliver at x=1 and slide vertically 200 px over many ticks
        // with tiny toward jitter (1 px left, dy 10 px each tick).
        let mut now = 0u64;
        // First tick outside
        let (g, shown) = reveal_gate_step(gate, 10, 100, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        // Enter sliver
        now += 16; let (g, shown) = reveal_gate_step(gate, 1, 110, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        for i in 0..20 {
            now += 16;
            let y = 120 + i * 10;
            // x jitter 0..1 (small toward)
            let x = if i % 2 == 0 { 1 } else { 0 };
            let (g, shown) = reveal_gate_step(gate.clone(), x, y, now, full, ABE_LEFT, sensitivity, dwell);
            gate = g;
            assert!(!shown, "graze tick {i} must not reveal");
        }
        // Even after dwell interval, still not revealed because sensitivity never reached
        now += dwell;
        let (g, shown) = reveal_gate_step(gate, 0, 320, now, full, ABE_LEFT, sensitivity, dwell);
        assert!(!shown, "graze must never reveal — along-edge dominant motion accumulates nothing");
        let _ = g;
    }

    #[test]
    fn reveal_gate_fly_through_overshoot_and_rebound_never_reveals() {
        // Ticket 112: fly-through — fast overshoot into the sliver and immediate rebound
        // accumulates enough toward travel to start dwell, but exits before dwell elapses.
        let full = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let mut gate = RevealGate::default();
        let sensitivity = crate::constants::window::REVEAL_SENSITIVITY_PX;
        let dwell = crate::constants::window::REVEAL_DWELL_MS;
        let mut now = 0u64;
        let (g, shown) = reveal_gate_step(gate, 40, 900, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        // Fast toward motion into sliver: 40 -> 1 (delta 39, capped 15) reaches sensitivity
        now += 16; let (g, shown) = reveal_gate_step(gate, 1, 900, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown); // dwell started but not elapsed
        // Hold briefly inside but not long enough
        now += 50; let (g, shown) = reveal_gate_step(gate, 0, 900, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        // Rebound out of band before dwell completes
        now += 16; let (g, shown) = reveal_gate_step(gate, 20, 900, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        // Even if we wait, the cancel-if-left has reset the gate
        now += dwell; let (g, shown) = reveal_gate_step(gate, 20, 900, now, full, ABE_LEFT, sensitivity, dwell);
        assert!(!shown, "fly-through must not reveal — dwell cancels on exit");
        let _ = g;
    }

    #[test]
    fn reveal_gate_deliberate_push_reveals_after_dwell() {
        // Ticket 112: deliberate push — sufficient toward-edge travel + 200 ms hold reveals.
        let full = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let mut gate = RevealGate::default();
        let sensitivity = crate::constants::window::REVEAL_SENSITIVITY_PX;
        let dwell = crate::constants::window::REVEAL_DWELL_MS;
        let mut now = 0u64;
        let (g, shown) = reveal_gate_step(gate, 30, 500, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        // Push into edge: 30 -> 1 delta 29 capped 15 accumulates past threshold (12)
        now += 16; let (g, shown) = reveal_gate_step(gate, 1, 500, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown, "dwell not yet elapsed");
        assert!(gate.accumulated >= sensitivity);
        assert!(gate.dwell_start_ms.is_some());
        let start = gate.dwell_start_ms.unwrap();
        // Just before dwell
        now = start + dwell - 1;
        let (g2, shown) = reveal_gate_step(gate.clone(), 0, 500, now, full, ABE_LEFT, sensitivity, dwell);
        assert!(!shown, "must not reveal before dwell");
        // At dwell
        now = start + dwell;
        let (_g3, shown) = reveal_gate_step(g2, 0, 500, now, full, ABE_LEFT, sensitivity, dwell);
        assert!(shown, "deliberate push must reveal after dwell");
        // Right edge mirrors left
        let full_right = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let mut gate = RevealGate::default();
        now = 0;
        let (g, _) = reveal_gate_step(gate, 2520, 500, now, full_right, ABE_RIGHT, sensitivity, dwell);
        gate = g;
        now += 16; let (g, _) = reveal_gate_step(gate, 2559, 500, now, full_right, ABE_RIGHT, sensitivity, dwell);
        gate = g;
        assert!(gate.accumulated >= sensitivity);
        now = gate.dwell_start_ms.unwrap() + dwell;
        let (_, shown) = reveal_gate_step(gate, 2560, 500, now, full_right, ABE_RIGHT, sensitivity, dwell);
        assert!(shown, "right edge deliberate push must also reveal");
    }

    #[test]
    fn reveal_gate_cross_monitor_transit_never_reveals() {
        // Ticket 112: cross-monitor transit — cursor sweeps through the 2 px sliver
        // while moving between monitors; exits within a frame or two, so dwell aborts.
        let full = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        let mut gate = RevealGate::default();
        let sensitivity = crate::constants::window::REVEAL_SENSITIVITY_PX;
        let dwell = crate::constants::window::REVEAL_DWELL_MS;
        let mut now = 0u64;
        // Transit from left monitor interior toward right monitor: x 2500 -> 2560+ across seam.
        // Use left-edge dock case where monitor is the left one (full left 0), so sliver is [0,2].
        // A transit across the seam at y inside strip that briefly hits the sliver:
        // Simulate moving rightward across left edge? Actually left edge sliver is at x=0,
        // a cross from right to left would hit it. Simulate leftward sweep 40->1-> -10 (outside)
        // where -10 is beyond monitor (still in virtual desktop but outside band).
        let (g, shown) = reveal_gate_step(gate, 40, 800, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; assert!(!shown);
        now += 16; let (g, shown) = reveal_gate_step(gate, 1, 800, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; // entered sliver
        assert!(!shown);
        now += 16; let (g, shown) = reveal_gate_step(gate, 20, 800, now, full, ABE_LEFT, sensitivity, dwell);
        gate = g; // exited within one tick (transit)
        assert!(!shown);
        now += dwell; let (g, shown) = reveal_gate_step(gate, 20, 800, now, full, ABE_LEFT, sensitivity, dwell);
        assert!(!shown, "cross-monitor transit must never reveal — cancel-if-left aborts dwell");
        let _ = g;
        // Right-edge dock transit: sweep left-to-right through [2558,2560]
        let full_right = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        let mut gate = RevealGate::default();
        now = 0;
        let (g, _) = reveal_gate_step(gate, 2540, 800, now, full_right, ABE_RIGHT, sensitivity, dwell);
        gate = g;
        now += 16; let (g, _) = reveal_gate_step(gate, 2559, 800, now, full_right, ABE_RIGHT, sensitivity, dwell);
        gate = g;
        now += 16; let (g, shown) = reveal_gate_step(gate, 2580, 800, now, full_right, ABE_RIGHT, sensitivity, dwell);
        assert!(!shown, "right-edge cross transit must also not reveal");
        let _ = g;
    }

    #[test]
    fn reveal_gate_trigger_zone_is_the_sliver_itself() {
        // Single size source: trigger band is exactly the sliver width.
        let full = RECT { left: 0, top: 0, right: 340, bottom: 1848 };
        assert!(in_reveal_band(0, 900, full, ABE_LEFT));
        assert!(in_reveal_band(2, 900, full, ABE_LEFT));
        assert!(!in_reveal_band(3, 900, full, ABE_LEFT));
        let right = RECT { left: 2220, top: 0, right: 2560, bottom: 1848 };
        assert!(in_reveal_band(2560, 900, right, ABE_RIGHT));
        assert!(!in_reveal_band(2557, 900, right, ABE_RIGHT));
    }

    #[test]
    #[ignore = "touches the real display configuration — run manually on a real session"]
    fn real_display_identity_resolves_or_falls_back_cleanly() {
        // Manual smoke for ticket 110: on any live session this either yields
        // an identity string or None (no usable EDID / query refusal) — never
        // a panic and never a colliding all-zero identity.
        let hwnd = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
        let hwnd = if hwnd.is_null() {
            unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetDesktopWindow() }
        } else {
            hwnd
        };
        if hwnd.is_null() {
            eprintln!("no window to probe");
            return;
        }
        match monitor_identity(hwnd) {
            Some(id) => println!("monitor identity: {id}"),
            None => println!("no usable identity — fallback path engaged"),
        }
        match monitor_key(hwnd) {
            Some(k) => println!("device-name key: {k}"),
            None => println!("no device name"),
        }
    }
}