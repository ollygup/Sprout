use std::{ffi::OsStr, os::windows::ffi::OsStrExt, path::Path};
use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::{SW_HIDE, SW_SHOWNORMAL}};

struct Invocation {
    operation: Vec<u16>,
    target: Vec<u16>,
    parameters: Option<Vec<u16>>,
    show: i32,
}

fn wide(text: &OsStr) -> Vec<u16> { text.encode_wide().chain(Some(0)).collect() }

fn invoke(call: &Invocation) -> isize {
    unsafe {
        ShellExecuteW(std::ptr::null_mut(), call.operation.as_ptr(), call.target.as_ptr(),
            call.parameters.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()),
            std::ptr::null(), call.show) as isize
    }
}

pub fn open_external(target: &str, what: &str) -> Result<(), String> {
    open_with(target, what, invoke)
}

fn open_with(target: &str, what: &str, native: impl FnOnce(&Invocation) -> isize) -> Result<(), String> {
    let result = native(&Invocation { operation: wide(OsStr::new("open")),
        target: wide(OsStr::new(target)), parameters: None, show: SW_SHOWNORMAL });
    if result > 32 { Ok(()) } else { Err(format!("Windows could not open {what} (error {result})")) }
}

pub fn launch_elevated(exe: &Path, args: &[&str]) -> Result<(), String> {
    elevate_with(exe, args, invoke)
}

fn elevate_with(exe: &Path, args: &[&str], native: impl FnOnce(&Invocation) -> isize) -> Result<(), String> {
    let code = native(&Invocation { operation: wide(OsStr::new("runas")), target: wide(exe.as_os_str()),
        parameters: Some(wide(OsStr::new(&args.join(" ")))), show: SW_HIDE });
    if code > 32 { Ok(()) } else { Err(match code {
        5 => "the UAC prompt was declined or blocked".to_string(),
        1223 => "the UAC prompt was cancelled".to_string(),
        _ => format!("Windows rejected the relaunch (error {code})"),
    }) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(wide: &[u16]) -> String { String::from_utf16_lossy(&wide[..wide.len()-1]) }

    #[test]
    fn external_open_uses_visible_default_handler_without_parameters() {
        assert!(open_with("https://example.test", "site", |call| {
            assert_eq!(text(&call.operation), "open");
            assert_eq!(text(&call.target), "https://example.test");
            assert!(call.parameters.is_none());
            assert_eq!(call.show, SW_SHOWNORMAL);
            33
        }).is_ok());
        assert_eq!(open_with("x", "site", |_| 32).unwrap_err(), "Windows could not open site (error 32)");
    }

    #[test]
    fn worker_relaunch_keeps_elevation_hidden_window_and_error_translation() {
        assert!(elevate_with(Path::new(r"C:\My Apps\Sprout.exe"), &["--worker", "--run", "run-id"], |call| {
            assert_eq!(text(&call.operation), "runas");
            assert_eq!(text(&call.target), r"C:\My Apps\Sprout.exe");
            assert_eq!(text(call.parameters.as_ref().unwrap()), "--worker --run run-id");
            assert_eq!(call.show, SW_HIDE);
            33
        }).is_ok());
        for (code, expected) in [(5, "declined or blocked"), (0, "error 0"), (32, "error 32")] {
            assert!(elevate_with(Path::new("x"), &[], |_| code).unwrap_err().contains(expected));
        }
        assert!(elevate_with(Path::new("x"), &[], |_| 1223).is_ok());
    }
}
