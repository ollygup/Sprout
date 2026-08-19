//! App-icon extraction (ticket 40): the per-candidate icon behind the Quick
//! Launch search results, as a `data:image/png` data URL.
//!
//! Glossary (docs/CONTEXT.md): a **candidate** is one app the installed-app
//! search found. Its `target` is either a Start Menu `.lnk` (the shell
//! resolves the shortcut to its target's icon) or an exe path; the icon comes
//! from the shell's icon cache/registry via `SHGetFileInfoW`.
//!
//! The whole pipeline is self-contained and leaks no handles: the `HICON`,
//! the DIB section, and the memory DC are all released on every path,
//! including failure ones (Drop guards). Icons are held in memory by the
//! frontend and never cached to disk.

use png::{BitDepth, ColorType, Encoder};

/// The icon for a candidate target (a `.lnk` or exe path), as a PNG data
/// URL. `None` when the target no longer exists or the shell has no icon for
/// it — the caller renders the row without one.
pub fn candidate_icon(target: &str) -> Option<String> {
    use windows_sys::Win32::UI::Shell::{
        SHFILEINFOW, SHGetFileInfoW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SHELLICONSIZE,
    };

    let path = std::path::Path::new(target);
    if !path.is_file() {
        // Uninstalled exe or odd target: no icon, gracefully.
        return None;
    }
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let wide = wide_string(path);
    let found = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON | SHGFI_SHELLICONSIZE,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    let _icon = IconGuard(info.hIcon);
    render_png(info.hIcon, 32)
}

/// Draws the icon into a 32×32 32bpp DIB section and encodes it as a PNG.
fn render_png(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON, size: u32) -> Option<String> {
    let rgba = icon_rgba(icon, size)?;
    let mut out: Vec<u8> = Vec::new();
    {
        let mut encoder = Encoder::new(&mut out, size, size);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
        writer.finish().ok()?;
    }
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    Some(format!("data:image/png;base64,{}", STANDARD.encode(&out)))
}

/// The icon's pixels as straight-alpha RGBA, top-down.
fn icon_rgba(icon: windows_sys::Win32::UI::WindowsAndMessaging::HICON, size: u32) -> Option<Vec<u8>> {
    use windows_sys::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CreateCompatibleDC, CreateDIBSection,
        DIB_RGB_COLORS, GetDC, SelectObject, HGDIOBJ,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{DI_NORMAL, DrawIconEx};

    let screen = unsafe { GetDC(std::ptr::null_mut()) };
    if screen.is_null() {
        return None;
    }
    let _screen = ScreenDcGuard(screen);
    let mem = unsafe { CreateCompatibleDC(screen) };
    if mem.is_null() {
        return None;
    }
    let _mem = DcGuard(mem);

    // A fresh 32×32 top-down 32bpp DIB: zeroed, straight-into-memory pixels.
    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = size as i32;
    bmi.bmiHeader.biHeight = -(size as i32);
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let dib = unsafe {
        CreateDIBSection(
            screen,
            &bmi,
            DIB_RGB_COLORS,
            &mut bits,
            std::ptr::null_mut(),
            0,
        )
    };
    if dib.is_null() || bits.is_null() {
        return None;
    }
    let _dib = GdiGuard(dib as HGDIOBJ);

    let previous = unsafe { SelectObject(mem, dib as HGDIOBJ) };
    let drawn = unsafe {
        DrawIconEx(
            mem,
            0,
            0,
            icon,
            size as i32,
            size as i32,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        )
    };
    if !previous.is_null() {
        unsafe { SelectObject(mem, previous) };
    }
    if drawn == 0 {
        return None;
    }

    // The DIB section's backing buffer is the pixels, bottom-up or top-down
    // per the header; this one is top-down BGRA with premultiplied alpha (as
    // DrawIconEx writes it) — swap to straight RGBA for the PNG.
    let stride = (size * 4) as usize;
    let raw = unsafe { std::slice::from_raw_parts(bits as *const u8, stride * size as usize) };
    Some(unpremultiply(raw, stride))
}

/// DrawIconEx writes premultiplied alpha into a 32bpp DIB; PNG wants straight
/// alpha, so each pixel is unpremultiplied (fully transparent pixels zero
/// out). Rows are copied top-down, converting BGRA → RGBA in the same pass.
fn unpremultiply(raw: &[u8], stride: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    for row in raw.chunks_exact(stride) {
        for px in row.chunks_exact(4) {
            let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
            if a == 0 {
                out.extend_from_slice(&[0, 0, 0, 0]);
                continue;
            }
            // c_out = c_in * 255 / a (the premultiplied channel, scaled back
            // to straight alpha).
            out.push(((r as u16 * 255) / a as u16).min(255) as u8);
            out.push(((g as u16 * 255) / a as u16).min(255) as u8);
            out.push(((b as u16 * 255) / a as u16).min(255) as u8);
            out.push(a);
        }
    }
    out
}

/// A wide NUL-terminated copy of `path`, for the shell call.
fn wide_string(path: &std::path::Path) -> Vec<u16> {
    path.to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

struct IconGuard(windows_sys::Win32::UI::WindowsAndMessaging::HICON);

impl Drop for IconGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::UI::WindowsAndMessaging::DestroyIcon;
        unsafe { DestroyIcon(self.0) };
    }
}

struct GdiGuard(windows_sys::Win32::Graphics::Gdi::HGDIOBJ);

impl Drop for GdiGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::DeleteObject;
        unsafe { DeleteObject(self.0) };
    }
}

struct DcGuard(windows_sys::Win32::Graphics::Gdi::HDC);

impl Drop for DcGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::DeleteDC;
        unsafe { DeleteDC(self.0) };
    }
}

struct ScreenDcGuard(windows_sys::Win32::Graphics::Gdi::HDC);

impl Drop for ScreenDcGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Graphics::Gdi::ReleaseDC;
        unsafe { ReleaseDC(std::ptr::null_mut(), self.0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unpremultiply_straightens_premultiplied_pixels() {
        // A mid-gray pixel at 50% alpha, premultiplied: 0x40 · 255 / 0x80.
        let raw = [0x40, 0x40, 0x40, 0x80];
        let out = unpremultiply(&raw, 4);
        assert_eq!(out, [0x7F, 0x7F, 0x7F, 0x80]);
    }

    #[test]
    fn unpremultiply_zeroes_fully_transparent_pixels() {
        let raw = [0xFF, 0x00, 0x00, 0x00];
        let out = unpremultiply(&raw, 4);
        assert_eq!(out, [0, 0, 0, 0]);
    }

#[test]
    fn unpremultiply_converts_bgra_to_rgba_top_down() {
        let raw = [0x11, 0x22, 0x33, 0xFF, 0xAA, 0xBB, 0xCC, 0xFF];
        let out = unpremultiply(&raw, 8);
        assert_eq!(out, [0x33, 0x22, 0x11, 0xFF, 0xCC, 0xBB, 0xAA, 0xFF]);
    }
}
