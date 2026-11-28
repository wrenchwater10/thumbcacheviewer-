// Custom dark chrome: one top strip holding our own title bar, menu bar and
// option bar, plus custom popup menus, the panel header strips and the status
// bar. Nothing here is drawn by Windows, so no light system chrome can leak in.

#![allow(dead_code)]

use crate::app::{
    self, MENU_COPY_IMAGE, MENU_EXPORT_IMAGE, MENU_OPEN, MENU_SETTINGS, MENU_TITLES, MENU_ZOOM_IN,
    MENU_ZOOM_OUT, MENU_ZOOM_RESET,
};
use crate::theme;
use crate::win::*;
use std::cell::{Cell, RefCell};

pub const TOP_CLASS: &str = "tcv_top";
pub const POPUP_CLASS: &str = "tcv_menupopup";
pub const PANELBAR_CLASS: &str = "tcv_panelbar";
pub const STATUS_CLASS: &str = "tcv_status";

pub const TITLE_H: i32 = 32;
pub const MENU_H: i32 = 26;
pub const TOOLBAR_H: i32 = 38;
pub const PANELBAR_H: i32 = 26;
pub const STATUS_H: i32 = 24;

const BTN_MIN: usize = 9001;
const BTN_MAX: usize = 9002;
const BTN_CLOSE: usize = 9003;

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

pub fn top_height(dpi: u32) -> i32 {
    scale(TITLE_H + MENU_H + TOOLBAR_H, dpi)
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

// ---------------------------------------------------------------- geometry

struct Layout {
    title: RECT,
    menu: RECT,
    toolbar: RECT,
    min: RECT,
    max: RECT,
    close: RECT,
    menu_items: Vec<(RECT, usize)>,
    buttons: Vec<(RECT, usize, &'static str)>,
}

unsafe fn layout(hwnd: HWND, dpi: u32) -> Layout {
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);

    let title_h = scale(TITLE_H, dpi);
    let menu_h = scale(MENU_H, dpi);
    let tool_h = scale(TOOLBAR_H, dpi);

    let title = RECT {
        left: 0,
        top: 0,
        right: rc.right,
        bottom: title_h,
    };
    let menu = RECT {
        left: 0,
        top: title_h,
        right: rc.right,
        bottom: title_h + menu_h,
    };
    let toolbar = RECT {
        left: 0,
        top: title_h + menu_h,
        right: rc.right,
        bottom: title_h + menu_h + tool_h,
    };

    let button_w = scale(46, dpi);
    let min = RECT {
        left: rc.right - button_w * 3,
        top: 0,
        right: rc.right - button_w * 2,
        bottom: title_h,
    };
    let max = RECT {
        left: rc.right - button_w * 2,
        top: 0,
        right: rc.right - button_w,
        bottom: title_h,
    };
    let close = RECT {
        left: rc.right - button_w,
        top: 0,
        right: rc.right,
        bottom: title_h,
    };

    let hdc = GetDC(hwnd);
    let mut menu_items = Vec::new();
    let mut x = scale(12, dpi);
    for (index, label) in MENU_TITLES.iter().enumerate() {
        let width = theme::measure_with_font(hdc, label, dpi, 9) + scale(20, dpi);
        menu_items.push((
            RECT {
                left: x,
                top: menu.top,
                right: x + width,
                bottom: menu.bottom,
            },
            index,
        ));
        x += width;
    }

    let pad = scale(13, dpi);
    let mut buttons = Vec::new();
    let mut bx = scale(10, dpi);
    for (group_index, group) in BUTTON_GROUP.iter().enumerate() {
        if group_index > 0 {
            bx += scale(7, dpi);
        }
        for (id, label) in group.iter() {
            let width = theme::measure_with_font(hdc, label, dpi, 9) + pad * 2;
            buttons.push((
                RECT {
                    left: bx,
                    top: toolbar.top + scale(5, dpi),
                    right: bx + width,
                    bottom: toolbar.bottom - scale(5, dpi),
                },
                *id,
                *label,
            ));
            bx += width + scale(2, dpi);
        }
    }
    ReleaseDC(hwnd, hdc);

    Layout {
        title,
        menu,
        toolbar,
        min,
        max,
        close,
        menu_items,
        buttons,
    }
}

fn inside(rc: &RECT, x: i32, y: i32) -> bool {
    x >= rc.left && x < rc.right && y >= rc.top && y < rc.bottom
}

unsafe fn region_at(hwnd: HWND, dpi: u32, x: i32, y: i32) -> (usize, usize) {
    let l = layout(hwnd, dpi);
    if inside(&l.min, x, y) {
        return (BTN_MIN, 0);
    }
    if inside(&l.max, x, y) {
        return (BTN_MAX, 0);
    }
    if inside(&l.close, x, y) {
        return (BTN_CLOSE, 0);
    }
    for (rect, index) in &l.menu_items {
        if inside(rect, x, y) {
            return (5000 + index, *index);
        }
    }
    for (rect, id, _) in &l.buttons {
        if inside(rect, x, y) {
            return (*id, 0);
        }
    }
    (0, 0)
}

// ----------------------------------------------------------------- painting

unsafe fn draw_glyph(hdc: HDC, kind: usize, rect: &RECT, color: u32) {
    let cx = (rect.left + rect.right) / 2;
    let cy = (rect.top + rect.bottom) / 2;
    match kind {
        BTN_MIN => {
            let r = RECT {
                left: cx - 5,
                top: cy,
                right: cx + 5,
                bottom: cy + 1,
            };
            theme::fill(hdc, &r, color);
        }
        BTN_MAX => {
            let r = RECT {
                left: cx - 5,
                top: cy - 5,
                right: cx + 5,
                bottom: cy - 4,
            };
            theme::fill(hdc, &r, color);
            let r = RECT {
                left: cx - 5,
                top: cy + 4,
                right: cx + 5,
                bottom: cy + 5,
            };
            theme::fill(hdc, &r, color);
            let r = RECT {
                left: cx - 5,
                top: cy - 5,
                right: cx - 4,
                bottom: cy + 5,
            };
            theme::fill(hdc, &r, color);
            let r = RECT {
                left: cx + 4,
                top: cy - 5,
                right: cx + 5,
                bottom: cy + 5,
            };
            theme::fill(hdc, &r, color);
        }
        _ => {
            for i in 0..9 {
                let r = RECT {
                    left: cx - 5 + i,
                    top: cy - 4 + i,
                    right: cx - 4 + i,
                    bottom: cy - 3 + i,
                };
                theme::fill(hdc, &r, color);
                let r = RECT {
                    left: cx + 4 - i,
                    top: cy - 4 + i,
                    right: cx + 5 - i,
                    bottom: cy - 3 + i,
                };
                theme::fill(hdc, &r, color);
            }
        }
    }
}

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);
    let dpi = GetDpiForWindow(hwnd);
    let l = layout(hwnd, dpi);
    let hover = HOVER.with(|h| h.get());
    let pressed = PRESSED.with(|p| p.get());

    // ---- title bar
    theme::fill(hdc, &l.title, theme::TITLE);
    let icon = LoadIconW(GetModuleHandleW(std::ptr::null()), 1 as usize as *const u16);
    let icon_size = scale(16, dpi);
    if !icon.is_null() {
        DrawIconEx(
            hdc,
            scale(10, dpi),
            (l.title.bottom - icon_size) / 2,
            icon,
            icon_size,
            icon_size,
            0,
            std::ptr::null_mut(),
            DI_NORMAL,
        );
    }
    let title = wide(&window_text(hwnd));
    theme::text_with_font(
        hdc,
        &String::from_utf16_lossy(&title[..title.len().saturating_sub(1)]),
        RECT {
            left: scale(34, dpi),
            top: 0,
            right: l.min.left - scale(8, dpi),
            bottom: l.title.bottom,
        },
        theme::TEXT,
        dpi,
        9,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
    );

    for (rect, id) in [(l.min, BTN_MIN), (l.max, BTN_MAX), (l.close, BTN_CLOSE)] {
        if hover == id || pressed == id {
            let color = if id == BTN_CLOSE {
                theme::CLOSE_HOVER
            } else {
                theme::HOVER
            };
            theme::fill(hdc, &rect, color);
        }
        draw_glyph(hdc, id, &rect, theme::TEXT);
    }

    // ---- menu bar
    theme::fill(hdc, &l.menu, theme::PANEL);
    for (rect, index) in &l.menu_items {
        let id = 5000 + index;
        if hover == id || pressed == id {
            theme::fill(hdc, rect, theme::HOVER);
        }
        theme::text_with_font(
            hdc,
            MENU_TITLES[*index],
            RECT {
                left: rect.left + scale(10, dpi),
                top: rect.top,
                right: rect.right - scale(10, dpi),
                bottom: rect.bottom,
            },
            theme::TEXT,
            dpi,
            9,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
    theme::hline(hdc, 0, l.menu.right, l.menu.bottom - 1, theme::BORDER);

    // ---- option bar
    theme::fill(hdc, &l.toolbar, theme::TOOLBAR);
    theme::hline(hdc, 0, l.toolbar.right, l.toolbar.bottom - 1, theme::BORDER);
    let mut x = scale(10, dpi);
    for (group_index, group) in BUTTON_GROUP.iter().enumerate() {
        if group_index > 0 {
            theme::vline(
                hdc,
                x + scale(3, dpi),
                l.toolbar.top + scale(9, dpi),
                l.toolbar.bottom - scale(10, dpi),
                theme::BORDER,
            );
            x += scale(7, dpi);
        }
        for (id, label) in group.iter() {
            let width = theme::measure_with_font(hdc, label, dpi, 9) + scale(13, dpi) * 2;
            let button = RECT {
                left: x,
                top: l.toolbar.top + scale(5, dpi),
                right: x + width,
                bottom: l.toolbar.bottom - scale(5, dpi),
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
                button,
                color,
                dpi,
                9,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
            x += width + scale(2, dpi);
        }
    }

    EndPaint(hwnd, &ps);
}

pub unsafe extern "system" fn top_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => 1,
        WM_PAINT => {
            paint(hwnd);
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
            let (id, _) = region_at(hwnd, dpi, x, y);
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
            let (id, _) = region_at(hwnd, dpi, x, y);
            let l = layout(hwnd, dpi);
            PRESSED.with(|p| p.set(id));
            InvalidateRect(hwnd, std::ptr::null(), 0);

            if inside(&l.title, x, y) && !(id == BTN_MIN || id == BTN_MAX || id == BTN_CLOSE) {
                // Let Windows move the window, since we removed the caption.
                ReleaseCapture();
                let pos = GetMessagePos();
                SendMessageW(
                    GetParent(hwnd),
                    WM_NCLBUTTONDOWN,
                    HTCAPTION as usize,
                    pos as isize,
                );
                return 0;
            }
            0
        }
        WM_LBUTTONDBLCLK => {
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let l = layout(hwnd, dpi);
            if inside(&l.title, x, y) {
                let parent = GetParent(hwnd);
                let style = GetWindowLongPtrW(parent, INDEX_STYLE);
                let command = if (style as u32 & WS_MAXIMIZE) != 0 {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                };
                ShowWindow(parent, command);
            }
            0
        }
        WM_LBUTTONUP => {
            let dpi = GetDpiForWindow(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let (id, menu_index) = region_at(hwnd, dpi, x, y);
            let pressed = PRESSED.with(|p| p.replace(0));
            InvalidateRect(hwnd, std::ptr::null(), 0);
            if pressed != id || id == 0 {
                return 0;
            }
            let parent = GetParent(hwnd);
            if id == BTN_MIN {
                ShowWindow(parent, SW_MINIMIZE);
            } else if id == BTN_MAX {
                let style = GetWindowLongPtrW(parent, INDEX_STYLE);
                let command = if (style as u32 & WS_MAXIMIZE) != 0 {
                    SW_RESTORE
                } else {
                    SW_MAXIMIZE
                };
                ShowWindow(parent, command);
            } else if id == BTN_CLOSE {
                PostMessageW(parent, WM_CLOSE, 0, 0);
            } else if id >= 5000 {
                open_menu(hwnd, menu_index);
            } else {
                PostMessageW(parent, WM_COMMAND, id, 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// -------------------------------------------------------------- popup menus

struct PopupState {
    entries: Vec<app::MenuEntry>,
    hover: usize,
    chosen: usize,
    dpi: u32,
    width: i32,
}

thread_local! {
    static POPUP: RefCell<Option<PopupState>> = const { RefCell::new(None) };
}

const ITEM_H: i32 = 26;
const SEP_H: i32 = 7;

unsafe fn popup_item_at(state: &PopupState, y: i32) -> Option<usize> {
    let mut offset = scale(6, state.dpi);
    for (index, entry) in state.entries.iter().enumerate() {
        if entry.separator {
            offset += scale(SEP_H, state.dpi);
            continue;
        }
        let height = scale(ITEM_H, state.dpi);
        if y >= offset && y < offset + height {
            return Some(index);
        }
        offset += height;
    }
    None
}

pub unsafe extern "system" fn popup_proc(
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

            POPUP.with(|cell| {
                let mut guard = cell.borrow_mut();
                let Some(state) = guard.as_mut() else {
                    return;
                };
                let dpi = state.dpi;
                theme::fill(hdc, &rc, theme::PANEL);
                theme::frame(hdc, &rc, theme::BORDER);

                let mut offset = scale(6, dpi);
                for (index, entry) in state.entries.iter().enumerate() {
                    if entry.separator {
                        theme::hline(
                            hdc,
                            scale(10, dpi),
                            rc.right - scale(10, dpi),
                            offset + scale(3, dpi),
                            theme::BORDER,
                        );
                        offset += scale(SEP_H, dpi);
                        continue;
                    }
                    let height = scale(ITEM_H, dpi);
                    let row = RECT {
                        left: scale(2, dpi),
                        top: offset,
                        right: rc.right - scale(2, dpi),
                        bottom: offset + height,
                    };
                    if state.hover == index {
                        theme::fill(hdc, &row, theme::HOVER);
                    }
                    if entry.checked {
                        theme::text_with_font(
                            hdc,
                            "\u{2713}",
                            RECT {
                                left: row.left + scale(8, dpi),
                                top: row.top,
                                right: row.left + scale(26, dpi),
                                bottom: row.bottom,
                            },
                            theme::TEXT,
                            dpi,
                            9,
                            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                        );
                    }
                    theme::text_with_font(
                        hdc,
                        entry.label,
                        RECT {
                            left: row.left + scale(28, dpi),
                            top: row.top,
                            right: row.right - scale(10, dpi),
                            bottom: row.bottom,
                        },
                        theme::TEXT,
                        dpi,
                        9,
                        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                    );
                    if !entry.accel.is_empty() {
                        theme::text_with_font(
                            hdc,
                            entry.accel,
                            RECT {
                                left: row.left,
                                top: row.top,
                                right: row.right - scale(12, dpi),
                                bottom: row.bottom,
                            },
                            theme::TEXT_DIM,
                            dpi,
                            9,
                            DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
                        );
                    }
                    offset += height;
                }
            });

            EndPaint(hwnd, &ps);
            0
        }
        WM_MOUSEMOVE => {
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let x = (lparam as i32) as i16 as i32;
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            if x < 0 || y < 0 || x >= rc.right || y >= rc.bottom {
                POPUP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        state.chosen = 0;
                    }
                });
                DestroyWindow(hwnd);
                return 0;
            }
            POPUP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    let hover = popup_item_at(state, y).unwrap_or(usize::MAX);
                    if state.hover != hover {
                        state.hover = hover;
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
            });
            0
        }
        WM_LBUTTONUP => {
            let y = ((lparam as i32) >> 16) as i16 as i32;
            POPUP.with(|cell| {
                if let Some(state) = cell.borrow_mut().as_mut() {
                    if let Some(index) = popup_item_at(state, y) {
                        state.chosen = state.entries[index].id;
                    }
                }
            });
            DestroyWindow(hwnd);
            0
        }
        WM_KEYDOWN => {
            let key = wparam as u16;
            if key == VK_ESCAPE {
                POPUP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        state.chosen = 0;
                    }
                });
                DestroyWindow(hwnd);
            } else if key == VK_RETURN {
                POPUP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        if let Some(entry) = state.entries.get(state.hover) {
                            state.chosen = entry.id;
                        }
                    }
                });
                DestroyWindow(hwnd);
            } else if key == VK_DOWN || key == VK_UP {
                POPUP.with(|cell| {
                    if let Some(state) = cell.borrow_mut().as_mut() {
                        let count = state.entries.len();
                        if count > 0 {
                            let mut next = state.hover;
                            for _ in 0..count {
                                next = if key == VK_DOWN {
                                    (next + 1) % count
                                } else {
                                    (next + count - 1) % count
                                };
                                if !state.entries[next].separator {
                                    break;
                                }
                            }
                            state.hover = next;
                        }
                    }
                });
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn open_menu(owner: HWND, index: usize) -> usize {
    let entries = app::menu_entries(index);
    if entries.is_empty() {
        return 0;
    }
    let hinst = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(owner);

    let hdc = GetDC(owner);
    let mut width = scale(170, dpi);
    let mut height = scale(12, dpi);
    for entry in &entries {
        if entry.separator {
            height += scale(SEP_H, dpi);
            continue;
        }
        height += scale(ITEM_H, dpi);
        let mut w = theme::measure_with_font(hdc, entry.label, dpi, 9) + scale(48, dpi);
        if !entry.accel.is_empty() {
            w += theme::measure_with_font(hdc, entry.accel, dpi, 9) + scale(24, dpi);
        }
        width = width.max(w);
    }
    ReleaseDC(owner, hdc);

    let mut anchor: RECT = std::mem::zeroed();
    let l = layout(owner, dpi);
    if let Some((rect, _)) = l.menu_items.get(index) {
        anchor = *rect;
    }
    let mut origin = POINT {
        x: anchor.left,
        y: anchor.bottom,
    };
    ClientToScreen(owner, &mut origin);

    POPUP.with(|cell| {
        *cell.borrow_mut() = Some(PopupState {
            entries,
            hover: usize::MAX,
            chosen: 0,
            dpi,
            width,
        });
    });

    // Owned by the top-level window, not by a child control.
    let main = GetParent(owner);
    let mut wclass = wide(POPUP_CLASS);
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
        wclass.as_mut_ptr(),
        wide("").as_ptr(),
        WS_POPUP | WS_VISIBLE,
        origin.x,
        origin.y,
        width + scale(4, dpi),
        height,
        main,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return 0;
    }

    SetCapture(hwnd);
    SetFocus(hwnd);
    InvalidateRect(hwnd, std::ptr::null(), 0);

    let mut msg: MSG = std::mem::zeroed();
    while IsWindow(hwnd) != 0 {
        let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
        if r <= 0 {
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
    if GetCapture() == hwnd {
        ReleaseCapture();
    }
    SetActiveWindow(main);

    let chosen = POPUP
        .with(|cell| cell.borrow().as_ref().map(|s| s.chosen))
        .unwrap_or(0);
    POPUP.with(|cell| *cell.borrow_mut() = None);

    if chosen != 0 {
        app::on_command(GetParent(owner), chosen);
    }
    chosen
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
