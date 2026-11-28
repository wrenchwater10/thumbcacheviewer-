// File/folder pickers, the dark settings window and the dark about box.

#![allow(dead_code)]

use crate::theme;
use crate::win::*;
use std::cell::RefCell;
use std::path::PathBuf;

pub const SETTINGS_CLASS: &str = "tcv_settings";

const IDOK: usize = 1;
const IDCANCEL: usize = 2;

const MODE_SETTINGS: u8 = 0;
const MODE_ABOUT: u8 = 1;
const MODE_MESSAGE: u8 = 2;

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(p: *mut core::ffi::c_void);
}

fn filter_block(items: &[(&str, &str)]) -> Vec<u16> {
    let mut out: Vec<u16> = Vec::new();
    for (label, pattern) in items {
        out.extend(label.encode_utf16());
        out.push(0);
        out.extend(pattern.encode_utf16());
        out.push(0);
    }
    out.push(0);
    out
}

fn split_multi(buf: &[u16]) -> Vec<PathBuf> {
    let mut parts: Vec<String> = Vec::new();
    let mut start = 0usize;
    for i in 0..buf.len() {
        if buf[i] == 0 {
            if i == start {
                break;
            }
            parts.push(String::from_utf16_lossy(&buf[start..i]));
            start = i + 1;
        }
    }
    match parts.len() {
        0 => Vec::new(),
        1 => vec![PathBuf::from(&parts[0])],
        _ => {
            let dir = PathBuf::from(&parts[0]);
            parts[1..].iter().map(|f| dir.join(f)).collect()
        }
    }
}

pub unsafe fn open_files(owner: HWND) -> Vec<PathBuf> {
    let mut buf = vec![0u16; 65536];
    let mut filter = filter_block(&[
        ("Thumbcache Databases", "thumbcache_*.db;*.db"),
        ("All Files", "*.*"),
    ]);
    let title = wide("Open Thumbcache Database");

    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = filter.as_mut_ptr();
    ofn.nFilterIndex = 1;
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_ALLOWMULTISELECT
        | OFN_HIDEREADONLY;

    if GetOpenFileNameW(&mut ofn) == 0 {
        return Vec::new();
    }
    split_multi(&buf)
}

pub unsafe fn save_file(
    owner: HWND,
    title: &str,
    items: &[(&str, &str)],
    default_ext: &str,
    default_name: &str,
) -> Option<PathBuf> {
    let mut buf = vec![0u16; 1024];
    for (i, c) in default_name.encode_utf16().enumerate() {
        if i + 1 < buf.len() {
            buf[i] = c;
        }
    }
    let mut filter = filter_block(items);
    let title = wide(title);
    let ext = wide(default_ext);

    let mut ofn: OPENFILENAMEW = std::mem::zeroed();
    ofn.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    ofn.hwndOwner = owner;
    ofn.lpstrFilter = filter.as_mut_ptr();
    ofn.nFilterIndex = 1;
    ofn.lpstrFile = buf.as_mut_ptr();
    ofn.nMaxFile = buf.len() as u32;
    ofn.lpstrTitle = title.as_ptr();
    ofn.lpstrDefExt = ext.as_ptr();
    ofn.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_OVERWRITEPROMPT | OFN_HIDEREADONLY;

    if GetSaveFileNameW(&mut ofn) == 0 {
        return None;
    }
    let mut len = 0usize;
    while len < buf.len() && buf[len] != 0 {
        len += 1;
    }
    if len == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}

pub unsafe fn pick_folder(owner: HWND) -> Option<PathBuf> {
    let title = wide("Select an output folder");
    let mut bi: BROWSEINFOW = std::mem::zeroed();
    bi.hwndOwner = owner;
    bi.lpszTitle = title.as_ptr();
    bi.ulFlags = BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE | BIF_EDITBOX;

    let pidl = SHBrowseForFolderW(&bi);
    if pidl.is_null() {
        return None;
    }
    let mut path = [0u16; 1024];
    let ok = SHGetPathFromIDListW(pidl, path.as_mut_ptr());
    CoTaskMemFree(pidl as *mut core::ffi::c_void);
    if ok == 0 {
        return None;
    }
    Some(PathBuf::from(from_wide(path.as_ptr())))
}

// ------------------------------------------------------------- modal plumbing

struct ModalState {
    edit: HWND,
    done: bool,
    value: Option<f32>,
    mode: u8,
}

thread_local! {
    static MODAL: RefCell<ModalState> = const { RefCell::new(ModalState {
        edit: std::ptr::null_mut(),
        done: false,
        value: None,
        mode: MODE_SETTINGS,
    }) };
}

unsafe fn get_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
    from_wide(buf.as_ptr())
}

unsafe fn draw_ownerdraw_button(lparam: LPARAM) {
    let dis = &*(lparam as *const DRAWITEMSTRUCT);
    let pressed = (dis.itemState & ODS_SELECTED) != 0;
    let focused = (dis.itemState & ODS_FOCUS) != 0;

    theme::fill(
        dis.hDC,
        &dis.rcItem,
        if pressed { theme::PRESS } else { theme::HOVER },
    );
    theme::frame(dis.hDC, &dis.rcItem, theme::BORDER);

    let text = get_text(dis.hwndItem);
    theme::text_with_font(
        dis.hDC,
        &text,
        dis.rcItem,
        theme::TEXT_BRIGHT,
        GetDpiForWindow(dis.hwndItem),
        9,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
    );

    if focused {
        let mut focus = dis.rcItem;
        focus.left += 3;
        focus.top += 3;
        focus.right -= 3;
        focus.bottom -= 3;
        DrawFocusRect(dis.hDC, &focus);
    }
}

pub unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => {
            let hdc: HDC = wparam as *mut core::ffi::c_void;
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            theme::fill(hdc, &rc, theme::PANEL);
            1
        }
        WM_CTLCOLORSTATIC => {
            let hdc: HDC = wparam as *mut core::ffi::c_void;
            SetTextColor(hdc, theme::TEXT);
            SetBkColor(hdc, theme::PANEL);
            theme::brush(theme::PANEL) as LRESULT
        }
        WM_CTLCOLOREDIT => {
            let hdc: HDC = wparam as *mut core::ffi::c_void;
            SetTextColor(hdc, theme::TEXT_BRIGHT);
            SetBkColor(hdc, theme::FIELD);
            theme::brush(theme::FIELD) as LRESULT
        }
        WM_CTLCOLORBTN => theme::brush(theme::PANEL) as LRESULT,
        WM_DRAWITEM => {
            draw_ownerdraw_button(lparam);
            1
        }
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if id == IDOK {
                if MODAL.with(|m| m.borrow().mode) == MODE_SETTINGS {
                    let edit = MODAL.with(|m| m.borrow().edit);
                    let text = get_text(edit);
                    let parsed = text.trim().replace(',', ".");
                    if let Ok(v) = parsed.parse::<f32>() {
                        MODAL.with(|m| m.borrow_mut().value = Some(v.clamp(0.01, 10.0)));
                    }
                }
                DestroyWindow(hwnd);
            } else if id == IDCANCEL {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            MODAL.with(|m| m.borrow_mut().done = true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn run_modal(owner: HWND, hwnd: HWND) {
    MODAL.with(|m| m.borrow_mut().done = false);
    EnableWindow(owner, 0);

    let mut msg: MSG = std::mem::zeroed();
    loop {
        let done = MODAL.with(|m| m.borrow().done);
        if done || IsWindow(hwnd) == 0 {
            break;
        }
        let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
        if r <= 0 {
            break;
        }
        if msg.message == WM_KEYDOWN {
            match msg.wParam as u16 {
                VK_RETURN => {
                    PostMessageW(hwnd, WM_COMMAND, IDOK, 0);
                    continue;
                }
                VK_ESCAPE => {
                    PostMessageW(hwnd, WM_COMMAND, IDCANCEL, 0);
                    continue;
                }
                _ => {}
            }
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    EnableWindow(owner, 1);
    SetActiveWindow(owner);
}

unsafe fn create_static(
    parent: HWND,
    hinst: HINSTANCE,
    text: &str,
    style: u32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    font: HFONT,
) -> HWND {
    let mut wtext = wide(text);
    let control = CreateWindowExW(
        0,
        wide("STATIC").as_ptr(),
        wtext.as_mut_ptr(),
        WS_CHILD | WS_VISIBLE | style,
        x,
        y,
        w,
        h,
        parent,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    SendMessageW(control, WM_SETFONT, font as usize, 1);
    control
}

unsafe fn create_button(
    parent: HWND,
    hinst: HINSTANCE,
    text: &str,
    id: usize,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    default: bool,
    font: HFONT,
) -> HWND {
    let mut wtext = wide(text);
    let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW as u32;
    if default {
        style |= BS_DEFPUSHBUTTON as u32;
    }
    let control = CreateWindowExW(
        0,
        wide("BUTTON").as_ptr(),
        wtext.as_mut_ptr(),
        style,
        x,
        y,
        w,
        h,
        parent,
        id as HMENU,
        hinst,
        std::ptr::null(),
    );
    SendMessageW(control, WM_SETFONT, font as usize, 1);
    control
}

unsafe fn create_dialog_window(
    owner: HWND,
    hinst: HINSTANCE,
    title: &str,
    w: i32,
    h: i32,
) -> HWND {
    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(owner, &mut rc);
    let x = rc.left + ((rc.right - rc.left) - w) / 2;
    let y = rc.top + ((rc.bottom - rc.top) - h) / 2;

    let mut wtitle = wide(title);
    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME,
        wide(SETTINGS_CLASS).as_ptr(),
        wtitle.as_mut_ptr(),
        WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_VISIBLE,
        x,
        y,
        w,
        h,
        owner,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );
    if !hwnd.is_null() {
        theme::enable_dark_mode(hwnd);
    }
    hwnd
}

// -------------------------------------------------------------------- settings

pub unsafe fn show_settings(owner: HWND, zoom_step: &mut f32) -> bool {
    let hinst = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(owner);

    MODAL.with(|m| {
        let mut state = m.borrow_mut();
        state.edit = std::ptr::null_mut();
        state.value = None;
        state.mode = MODE_SETTINGS;
    });

    let hwnd = create_dialog_window(
        owner,
        hinst,
        "Settings",
        scale(340, dpi),
        scale(170, dpi),
    );
    if hwnd.is_null() {
        return false;
    }

    let font = theme::font(dpi, 9);
    create_static(
        hwnd,
        hinst,
        "Zoom increment (0.01 \u{2013} 10)",
        SS_TEXT_LEFT,
        scale(16, dpi),
        scale(20, dpi),
        scale(300, dpi),
        scale(20, dpi),
        font,
    );

    let initial = format!("{}", *zoom_step);
    let mut wtext = wide(&initial);
    let edit = CreateWindowExW(
        0,
        wide("EDIT").as_ptr(),
        wtext.as_mut_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
        scale(16, dpi),
        scale(44, dpi),
        scale(100, dpi),
        scale(24, dpi),
        hwnd,
        (100 as usize) as HMENU,
        hinst,
        std::ptr::null(),
    );
    SendMessageW(edit, WM_SETFONT, font as usize, 1);
    theme::untheme(edit);

    create_static(
        hwnd,
        hinst,
        "Saved to settings.txt next to the executable.",
        SS_TEXT_LEFT,
        scale(16, dpi),
        scale(76, dpi),
        scale(300, dpi),
        scale(20, dpi),
        font,
    );

    create_button(
        hwnd,
        hinst,
        "OK",
        IDOK,
        scale(168, dpi),
        scale(112, dpi),
        scale(70, dpi),
        scale(26, dpi),
        true,
        font,
    );
    create_button(
        hwnd,
        hinst,
        "Cancel",
        IDCANCEL,
        scale(248, dpi),
        scale(112, dpi),
        scale(70, dpi),
        scale(26, dpi),
        false,
        font,
    );

    MODAL.with(|m| m.borrow_mut().edit = edit);

    SetFocus(edit);
    SendMessageW(edit, EM_SETSEL, 0, -1);

    run_modal(owner, hwnd);

    let value = MODAL.with(|m| m.borrow().value);
    if let Some(v) = value {
        *zoom_step = v;
        true
    } else {
        false
    }
}

// --------------------------------------------------------------------- message

/// Themed replacement for MessageBoxW: same modal behaviour, dark window.
pub unsafe fn show_message(owner: HWND, text: &str, warning: bool) {
    let hinst = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(owner);

    MODAL.with(|m| {
        let mut state = m.borrow_mut();
        state.edit = std::ptr::null_mut();
        state.value = None;
        state.mode = MODE_MESSAGE;
    });

    let title = if warning { "Warning" } else { "Thumbcache Viewer" };
    let hwnd = create_dialog_window(
        owner,
        hinst,
        title,
        scale(440, dpi),
        scale(212, dpi),
    );
    if hwnd.is_null() {
        let mut wtext = wide(text);
        let mut wtitle = wide(title);
        MessageBoxW(owner, wtext.as_mut_ptr(), wtitle.as_mut_ptr(), MB_OK);
        return;
    }

    let font = theme::font(dpi, 9);
    create_static(
        hwnd,
        hinst,
        text,
        SS_TEXT_LEFT,
        scale(18, dpi),
        scale(18, dpi),
        scale(404, dpi),
        scale(126, dpi),
        font,
    );
    create_button(
        hwnd,
        hinst,
        "OK",
        IDOK,
        scale(356, dpi),
        scale(156, dpi),
        scale(66, dpi),
        scale(26, dpi),
        true,
        font,
    );

    run_modal(owner, hwnd);
}

// ----------------------------------------------------------------------- about

pub unsafe fn show_about(owner: HWND) {
    let hinst = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(owner);

    MODAL.with(|m| {
        let mut state = m.borrow_mut();
        state.edit = std::ptr::null_mut();
        state.value = None;
        state.mode = MODE_ABOUT;
    });

    let hwnd = create_dialog_window(owner, hinst, "About", scale(410, dpi), scale(220, dpi));
    if hwnd.is_null() {
        return;
    }

    let font = theme::font(dpi, 9);
    let text = format!(
        "Thumbcache Viewer {}\n\nThemed like a photo editor, but it just reads Windows thumbnail\ncache databases and shows the images inside them.\n\nRewritten in Rust.",
        env!("CARGO_PKG_VERSION")
    );
    create_static(
        hwnd,
        hinst,
        &text,
        SS_TEXT_LEFT,
        scale(18, dpi),
        scale(16, dpi),
        scale(374, dpi),
        scale(140, dpi),
        font,
    );
    create_button(
        hwnd,
        hinst,
        "OK",
        IDOK,
        scale(326, dpi),
        scale(164, dpi),
        scale(66, dpi),
        scale(26, dpi),
        true,
        font,
    );

    run_modal(owner, hwnd);
}
