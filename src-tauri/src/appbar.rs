//! Win32 AppBar docking for the Quick Launch window (ticket 53).
//!
//! The docked form is a taskbar-style AppBar registered with `ABM_NEW` and
//! released with `ABM_REMOVE`. Auto-hide (the default) delegates the
//! slide-to-a-sliver / hover-to-reveal behavior to the OS itself via
//! `ABM_AUTOHIDE` — the window is otherwise fully owned by the system while
//! hidden and we never fight it. Fixed mode keeps the strip permanently
//! reserved; maximized windows on the docked edge shrink to accommodate
//! (ADR-0011).
//!
//! The syscall surface (`SHAppBarMessage`) is not unit-testable on CI; the
//! geometry math is factored into pure functions tested below.

use std::mem::size_of;

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFOEXW, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::UI::Shell::{
    SHAppBarMessage, ABE_LEFT, ABE_RIGHT, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETAUTOHIDEBAR,
    ABM_SETPOS, APPBARDATA,
};

/// The docked strip's width in physical pixels — the same design-system scale
/// as the floating window's width, slim against the full work-area height.
pub const DOCK_WIDTH: i32 = 320;

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

/// The monitor's work area (the screen area minus the taskbar) for the
/// monitor the window currently sits on.
pub fn work_area(hwnd: HWND) -> Option<RECT> {
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
        Some(info.monitorInfo.rcWork)
    }
}

/// A stable per-monitor key for the dock's memory: the monitor's device name
/// (e.g. `\\.\DISPLAY1`), which survives display rearrangements and reboots.
pub fn monitor_key(hwnd: HWND) -> Option<String> {
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
        let name = String::from_utf16_lossy(&info.szDevice);
        let name = name.trim_end_matches('\0').to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}

/// Registers the window as an AppBar at `edge` sized to `desired`, applying
/// auto-hide when requested. Returns the system-corrected final rectangle the
/// window must be placed in.
pub fn register(hwnd: HWND, edge: u32, desired: RECT, autohide: bool) -> Result<RECT, String> {
    unsafe {
        let mut data = appbar_data(hwnd, edge, desired);
        if SHAppBarMessage(ABM_NEW, &mut data) == 0 {
            return Err("the system rejected the app bar registration".into());
        }
        if SHAppBarMessage(ABM_QUERYPOS, &mut data) == 0 {
            return Err("the system rejected the app bar position query".into());
        }
        let rect = data.rc;
        let mut set = appbar_data(hwnd, edge, rect);
        SHAppBarMessage(ABM_SETPOS, &mut set);
        if autohide {
            set_autohide(hwnd, true);
        }
        Ok(rect)
    }
}

/// Re-applies the AppBar's position after an edge switch: re-queries the new
/// edge's rect and commits it, keeping the window's auto-hide state.
pub fn reposition(hwnd: HWND, edge: u32, desired: RECT, autohide: bool) -> Result<RECT, String> {
    unsafe {
        let mut data = appbar_data(hwnd, edge, desired);
        if SHAppBarMessage(ABM_QUERYPOS, &mut data) == 0 {
            return Err("the system rejected the app bar position query".into());
        }
        let rect = data.rc;
        let mut set = appbar_data(hwnd, edge, rect);
        SHAppBarMessage(ABM_SETPOS, &mut set);
        if autohide {
            set_autohide(hwnd, true);
        }
        Ok(rect)
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

/// Enables or disables the auto-hide behavior: when enabled, the system hides
/// the window and slides it out on hover at the edge (taskbar-like).
pub fn set_autohide(hwnd: HWND, enabled: bool) {
    unsafe {
        let mut data = appbar_data(hwnd, ABE_LEFT, empty_rect());
        data.lParam = if enabled { 1 } else { 0 };
        SHAppBarMessage(ABM_SETAUTOHIDEBAR, &mut data);
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

fn appbar_data(hwnd: HWND, edge: u32, rc: RECT) -> APPBARDATA {
    APPBARDATA {
        cbSize: size_of::<APPBARDATA>() as u32,
        hWnd: hwnd,
        uCallbackMessage: 0,
        uEdge: edge,
        rc,
        lParam: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_edge_strips_the_work_area() {
        let work = RECT {
            left: 0,
            top: 40,
            right: 1920,
            bottom: 1040,
        };
        let rect = appbar_rect(work, ABE_LEFT, DOCK_WIDTH);
        assert_eq!(rect.left, 0);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.right, DOCK_WIDTH);
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
        let rect = appbar_rect(work, ABE_RIGHT, DOCK_WIDTH);
        assert_eq!(rect.left, 1920 - DOCK_WIDTH);
        assert_eq!(rect.top, 40);
        assert_eq!(rect.right, 1920);
        assert_eq!(rect.bottom, 1040);
    }

    #[test]
    fn edge_constant_maps_settings_strings() {
        assert_eq!(edge_constant("left"), Some(ABE_LEFT));
        assert_eq!(edge_constant("right"), Some(ABE_RIGHT));
        assert_eq!(edge_constant("top"), None);
    }
}