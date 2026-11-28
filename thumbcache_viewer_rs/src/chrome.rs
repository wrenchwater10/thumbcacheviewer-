// Photoshop-style dark chrome: the option bar along the top, the panel header
// strips above the list and the preview, and the status bar at the bottom.
// All three are custom-drawn child windows so they never inherit the light
// system colours.

#![allow(dead_code)]

use crate::app::{
    MENU_COPY_IMAGE, MENU_EXPORT_IMAGE, MENU_OPEN, MENU_SETTINGS, MENU_ZOOM_IN, MENU_ZOOM_OUT,
    MENU_ZOOM_RESET,
};
use crate::theme;
use crate::win::*;
use std::cell::Cell;

pub const TOOLBAR_CLASS: &str = "tcv_toolbar";
pub const PANELBAR_CLASS: &str = "tcv_panelbar";
pub const STATUS_CLASS: &str = "tcv_status";

pub const TOOLBAR_HEIGHT: i32 = 38;
pub const PANELBAR_HEIGHT: i32 = 26;
pub const STATUS_HEIGHT: i32 = 24;

const BUTTON_GROUP: &[&[(usize, &str)]] = &[
    &[(MENU_OPEN, "Open")],
    &[
        (MENU_COPY_IMAGE, "Copy Image"),
        (MENU_EXPORT_IMAGE, "Export Image"),
    ],
    &[
        (MENU_ZOOM_OUT, "\u{2013}"),
        (MENU_ZOOM_RESET, "Fit"),
        (MENU_ZOOM_IN, "+"),
    ],
    &[(MENU_SETTINGS, "Settings")],
];

thread_local! {
    static HOVER: Cell<usize> = const { Cell::new(0) };
    static PRESSED: Cell<usize> = const { Cell::new(0) };
}

pub unsafe fn set_text(hwnd: HWND, text: &str) {
    let mut wtext = wide(text);
    SetWindowTextW(hwnd, wtext.as_mut_ptr());
    InvalidateRect(hwnd, std::ptr::null(), 0);
}

unsafe fn window_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    from_wide(buf.as_ptr())
}

// --------------------------------------------------------------------- toolbar

unsafe fn button_rects(hwnd: HWND, dpi: u32) -> Vec<(RECT, usize, &'static str)> {
    let pad = scale(13, dpi);
    let height = scale(TOOLBAR_HEIGHT, dpi) - scale(10, dpi);
    let top = scale(5, dpi);
    let mut x = scale(10, dpi);
    let mut out: Vec<(RECT, usize, &'static str)> = Vec::new();

    let hdc = GetDC(hwnd);
    for (group_index, group) in BUTTON_GROUP.iter().enumerate() {
        if group_index > 0 {
            x += scale(7, dpi);
        }
        for (id, label) in group.iter() {
            let width = theme::measure_with_font(hdc, label, dpi, 9) + pad * 2;
            let rect = RECT {
                left: x,
                top,
                right: x + width,
                bottom: top + height,
            };
            out.push((rect, *id, *label));
            x += width + scale(2, dpi);
        }
    }
    ReleaseDC(hwnd, hdc);
    out
}

unsafe fn hit_test(hwnd: HWND, dpi: u32, x: i32, y: i32) -> usize {
    for (rect, id, _) in button_rects(hwnd, dpi) {
        if x >= rect.left && x < rect.right && y >= rect.top && y < rect.bottom {
            return id;
        }
    }
    0
}

pub unsafe extern "system" fn toolbar_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let dpi = GetDpiForWindow(hwnd);

            theme::fill(hdc, &rc, theme::TOOLBAR);
            theme::hline(hdc, 0, rc.right, rc.bottom - 1, theme::BORDER);

            let hover = HOVER.with(|h| h.get());
            let pressed = PRESSED.with(|p| p.get());

            let mut x = scale(10, dpi);
            for (group_index, group) in BUTTON_GROUP.iter().enumerate() {
                if group_index > 0 {
                    theme::vline(hdc, x + scale(3, dpi), scale(9, dpi), rc.bottom - scale(10, dpi), theme::BORDER);
                    x += scale(7, dpi);
                }
                for (id, label) in group.iter() {
                    let width = theme::measure_with_font(hdc, label, dpi, 9) + scale(13, dpi) * 2;
                    let button = RECT {
                        left: x,
                        top: scale(5, dpi),
                        right: x + width,
                        bottom: rc.bottom - scale(5, dpi),
                    };
                    if *id == pressed {
                        theme::fill(hdc, &button, theme::PRESS);
                    } else if *id == hover {
                        theme::fill(hdc, &button, theme::HOVER);
                    }
                    let color = if *id == hover || *id == pressed {
                        theme::TEXT_BRIGHT
                    } else {
                        theme::TEXT
                    };
                    theme::text_with_font(
                        hdc,
                        label,
                        RECT {
                            left: button.left,
                            top: button.top,
                            right: button.right,
                            bottom: button.bottom,
                        },
                        color,
                        dpi,
                        9,
                        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                    );
                    x += width + scale(2, dpi);
                }
            }

            EndPaint(hwnd, &ps);
            0
        }
        WM_SETTEXT => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            result
        }
        WM_MOUSEMOVE => {
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let id = hit_test(hwnd, dpi, x, y);
            if HOVER.with(|h| h.replace(id)) != id {
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            let mut track: TRACKMOUSEEVENT = std::mem::zeroed();
            track.cbSize = std::mem::size_of::<TRACKMOUSEEVENT>() as u32;
            track.dwFlags = TME_LEAVE;
            track.hwndTrack = hwnd;
            TrackMouseEvent(&mut track);
            0
        }
        WM_MOUSELEAVE => {
            HOVER.with(|h| h.set(0));
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_LBUTTONDOWN => {
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let id = hit_test(hwnd, dpi, x, y);
            PRESSED.with(|p| p.set(id));
            SetCapture(hwnd);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            let pressed = PRESSED.with(|p| p.replace(0));
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            InvalidateRect(hwnd, std::ptr::null(), 0);
            if pressed != 0 && hit_test(hwnd, dpi, x, y) == pressed {
                PostMessageW(GetParent(hwnd), WM_COMMAND, pressed, 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// -------------------------------------------------------------------- panelbar

pub unsafe extern "system" fn panelbar_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let dpi = GetDpiForWindow(hwnd);

            theme::fill(hdc, &rc, theme::HEADER);
            theme::hline(hdc, 0, rc.right, rc.bottom - 1, theme::BORDER);

            let label = window_text(hwnd);
            if !label.is_empty() {
                theme::text_with_font(
                    hdc,
                    &label.to_uppercase(),
                    RECT {
                        left: scale(12, dpi),
                        top: 0,
                        right: rc.right - scale(8, dpi),
                        bottom: rc.bottom - 1,
                    },
                    theme::TEXT_DIM,
                    dpi,
                    8,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }

            EndPaint(hwnd, &ps);
            0
        }
        WM_SETTEXT => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            result
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ----------------------------------------------------------------------- status

pub unsafe extern "system" fn status_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            let dpi = GetDpiForWindow(hwnd);

            theme::fill(hdc, &rc, theme::TOOLBAR);
            theme::hline(hdc, 0, rc.right, 0, theme::BORDER);

            let text = window_text(hwnd);
            let mut parts = text.splitn(2, '\t');
            let left = parts.next().unwrap_or("");
            let right = parts.next().unwrap_or("");

            theme::text_with_font(
                hdc,
                left,
                RECT {
                    left: scale(12, dpi),
                    top: 1,
                    right: rc.right / 2,
                    bottom: rc.bottom,
                },
                theme::TEXT_DIM,
                dpi,
                9,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            if !right.is_empty() {
                theme::text_with_font(
                    hdc,
                    right,
                    RECT {
                        left: rc.right / 2,
                        top: 1,
                        right: rc.right - scale(12, dpi),
                        bottom: rc.bottom,
                    },
                    theme::TEXT_DIM,
                    dpi,
                    9,
                    DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                );
            }

            EndPaint(hwnd, &ps);
            0
        }
        WM_SETTEXT => {
            let result = DefWindowProcW(hwnd, msg, wparam, lparam);
            InvalidateRect(hwnd, std::ptr::null(), 0);
            result
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
