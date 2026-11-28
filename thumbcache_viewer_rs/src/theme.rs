// Dark theme: palette, cached GDI fonts and brushes, small painting helpers and
// the opt-in that makes the native window chrome (title bar and menu bar) dark.

#![allow(dead_code)]

use crate::win::*;
use std::cell::RefCell;

pub const fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

// Photoshop-like greys: the canvas and frame are mid grey, panels are darker,
// panel headers and the option bar are lighter so they read as raised chrome.
pub const BG: u32 = rgb(0x32, 0x32, 0x32);
pub const CANVAS: u32 = rgb(0x28, 0x28, 0x28);
pub const TOOLBAR: u32 = rgb(0x2B, 0x2B, 0x2B);
pub const PANEL: u32 = rgb(0x26, 0x26, 0x26);
pub const HEADER: u32 = rgb(0x38, 0x38, 0x38);
pub const BORDER: u32 = rgb(0x18, 0x18, 0x18);
pub const TEXT: u32 = rgb(0xD8, 0xD8, 0xD8);
pub const TEXT_DIM: u32 = rgb(0x8E, 0x8E, 0x8E);
pub const TEXT_BRIGHT: u32 = rgb(0xFF, 0xFF, 0xFF);
pub const HOVER: u32 = rgb(0x3E, 0x3E, 0x3E);
pub const PRESS: u32 = rgb(0x1C, 0x1C, 0x1C);
pub const ACCENT: u32 = rgb(0x2D, 0x7D, 0xD2);
pub const FIELD: u32 = rgb(0x1E, 0x1E, 0x1E);
pub const IMAGE_FRAME: u32 = rgb(0x0F, 0x0F, 0x0F);
pub const SELECT: u32 = rgb(0x2E, 0x5F, 0x8E);
pub const TITLE: u32 = rgb(0x1E, 0x1E, 0x1E);
pub const ROW_ALT: u32 = rgb(0x2A, 0x2A, 0x2A);
pub const SCROLL_THUMB: u32 = rgb(0x4A, 0x4A, 0x4A);
pub const CLOSE_HOVER: u32 = rgb(0xC4, 0x2B, 0x1C);

thread_local! {
    static BRUSHES: RefCell<Vec<(u32, HBRUSH)>> = const { RefCell::new(Vec::new()) };
    static FONTS: RefCell<Vec<((i32, u32), HFONT)>> = const { RefCell::new(Vec::new()) };
}

pub unsafe fn brush(color: u32) -> HBRUSH {
    BRUSHES.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, handle)) = cache.iter().find(|(c, _)| *c == color) {
            return *handle;
        }
        let handle = CreateSolidBrush(color);
        cache.push((color, handle));
        handle
    })
}

pub unsafe fn font(dpi: u32, points: i32) -> HFONT {
    FONTS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some((_, handle)) = cache.iter().find(|((p, d), _)| *p == points && *d == dpi) {
            return *handle;
        }
        let height = -(((points as i64) * (dpi as i64) + 36) / 72) as i32;
        let face = wide("Segoe UI");
        let handle = CreateFontW(
            height,
            0,
            0,
            0,
            FW_NORMAL as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            (DEFAULT_PITCH | FF_DONTCARE) as u32,
            face.as_ptr(),
        );
        cache.push(((points, dpi), handle));
        handle
    })
}

pub unsafe fn fill(hdc: HDC, rc: &RECT, color: u32) {
    FillRect(hdc, rc, brush(color));
}

pub unsafe fn frame(hdc: HDC, rc: &RECT, color: u32) {
    FrameRect(hdc, rc, brush(color));
}

pub unsafe fn hline(hdc: HDC, x0: i32, x1: i32, y: i32, color: u32) {
    let rc = RECT {
        left: x0,
        top: y,
        right: x1,
        bottom: y + 1,
    };
    FillRect(hdc, &rc, brush(color));
}

pub unsafe fn vline(hdc: HDC, x: i32, y0: i32, y1: i32, color: u32) {
    let rc = RECT {
        left: x,
        top: y0,
        right: x + 1,
        bottom: y1,
    };
    FillRect(hdc, &rc, brush(color));
}

pub unsafe fn text(hdc: HDC, s: &str, rc: RECT, color: u32, flags: u32) {
    let mut rc = rc;
    let mut wtext = wide(s);
    SetBkMode(hdc, TRANSPARENT as i32);
    SetTextColor(hdc, color);
    DrawTextW(hdc, wtext.as_mut_ptr(), -1, &mut rc, flags);
}

/// Draws text with the cached Segoe UI face, restoring the previous font.
pub unsafe fn text_with_font(
    hdc: HDC,
    s: &str,
    rc: RECT,
    color: u32,
    dpi: u32,
    points: i32,
    flags: u32,
) {
    let handle = font(dpi, points);
    let old = SelectObject(hdc, handle as HGDIOBJ);
    text(hdc, s, rc, color, flags);
    SelectObject(hdc, old);
}

pub unsafe fn measure_with_font(hdc: HDC, s: &str, dpi: u32, points: i32) -> i32 {
    let handle = font(dpi, points);
    let old = SelectObject(hdc, handle as HGDIOBJ);
    let mut size: SIZE = std::mem::zeroed();
    let mut wtext = wide(s);
    GetTextExtentPoint32W(hdc, wtext.as_mut_ptr(), (wtext.len() - 1) as i32, &mut size);
    SelectObject(hdc, old);
    size.cx
}

// ---------------------------------------------------------------- dark chrome

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        hwnd: HWND,
        attribute: u32,
        value: *const core::ffi::c_void,
        size: u32,
    ) -> i32;
}

/// Opts the process into the Windows dark shell styling, which is what makes the
/// title bar and the native menu bar dark on Windows 10 1903+ and Windows 11.
/// Must run once, before any window exists, for the menu bar and the common
/// controls to be created in dark styling.
pub unsafe fn init_dark_mode() {
    let uxtheme = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if uxtheme.is_null() {
        return;
    }
    // 135 = SetPreferredAppMode (2 = ForceDark), 136 = FlushMenuThemes.
    if let Some(proc) = GetProcAddress(uxtheme, 135usize as *const u8) {
        let set_mode: unsafe extern "system" fn(i32) -> i32 = std::mem::transmute(proc);
        set_mode(2);
    }
    if let Some(proc) = GetProcAddress(uxtheme, 136usize as *const u8) {
        let flush: unsafe extern "system" fn() = std::mem::transmute(proc);
        flush();
    }
}

/// Per window: allow dark styling and ask DWM for a dark title bar. Called for
/// the frame and every dialog, and again after the window is shown because some
/// builds only honour the caption attribute once the window is visible.
pub unsafe fn enable_dark_mode(hwnd: HWND) {
    allow_dark(hwnd);
    dark_title_bar(hwnd);
}

/// 133 = AllowDarkModeForWindow; older builds simply do not export it.
pub unsafe fn allow_dark(hwnd: HWND) {
    let uxtheme = GetModuleHandleW(wide("uxtheme.dll").as_ptr());
    if uxtheme.is_null() {
        return;
    }
    if let Some(proc) = GetProcAddress(uxtheme, 133usize as *const u8) {
        let allow: unsafe extern "system" fn(HWND, i32) -> i32 = std::mem::transmute(proc);
        allow(hwnd, 1);
    }
}

pub unsafe fn dark_title_bar(hwnd: HWND) {
    let on: i32 = 1;
    let ptr = &on as *const i32 as *const core::ffi::c_void;
    // 20 on current builds, 19 on 1809-2004.
    if DwmSetWindowAttribute(hwnd, 20, ptr, 4) != 0 {
        DwmSetWindowAttribute(hwnd, 19, ptr, 4);
    }
}

/// Applies the internal Explorer dark theme, which is what makes a control's
/// scrollbars dark instead of the classic light ones.
pub unsafe fn darken_control(hwnd: HWND) {
    let uxtheme = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if uxtheme.is_null() {
        return;
    }
    if let Some(proc) = GetProcAddress(uxtheme, b"SetWindowTheme\0".as_ptr()) {
        let set_theme: unsafe extern "system" fn(HWND, *const u16, *const u16) -> i32 =
            std::mem::transmute(proc);
        let name = wide("DarkMode_Explorer");
        set_theme(hwnd, name.as_ptr(), std::ptr::null());
    }
}

unsafe extern "system" fn darken_scrollbar_child(hwnd: HWND, _param: LPARAM) -> i32 {
    let mut buf = [0u16; 64];
    let len = GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    if len > 0 {
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        if class.eq_ignore_ascii_case("ScrollBar") {
            // The scrollbar needs both the dark-mode allowance and the theme.
            allow_dark(hwnd);
            darken_control(hwnd);
        }
    }
    1
}

/// Themes only the scrollbar children of a control, leaving its own drawing to
/// us (the header and the rows are painted by hand).
pub unsafe fn darken_scrollbars(parent: HWND) {
    EnumChildWindows(parent, Some(darken_scrollbar_child), 0);
}

/// Removes the visual-style theme from a control so it honours our own colours.
pub unsafe fn untheme(hwnd: HWND) {
    let uxtheme = LoadLibraryW(wide("uxtheme.dll").as_ptr());
    if uxtheme.is_null() {
        return;
    }
    if let Some(proc) = GetProcAddress(uxtheme, b"SetWindowTheme\0".as_ptr()) {
        let set_theme: unsafe extern "system" fn(HWND, *const u16, *const u16) -> i32 =
            std::mem::transmute(proc);
        let empty = wide("");
        set_theme(hwnd, empty.as_ptr(), empty.as_ptr());
    }
}
