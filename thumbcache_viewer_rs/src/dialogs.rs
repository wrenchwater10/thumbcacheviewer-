// File/folder pickers, the settings dialog and the about box.

#![allow(dead_code)]

use crate::win::*;
use std::cell::RefCell;
use std::path::PathBuf;

pub const SETTINGS_CLASS: &str = "tcv_settings";

const IDOK: usize = 1;
const IDCANCEL: usize = 2;

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
    Some(PathBuf::from(unsafe { from_wide(path.as_ptr()) }))
}

// ------------------------------------------------------------------- settings

struct SettingsState {
    edit: HWND,
    done: bool,
    value: Option<f32>,
    initial: f32,
}

thread_local! {
    static SETTINGS: RefCell<SettingsState> = const { RefCell::new(SettingsState {
        edit: std::ptr::null_mut(),
        done: false,
        value: None,
        initial: 0.5,
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

pub unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = wparam & 0xFFFF;
            if id == IDOK {
                let text = SETTINGS.with(|s| get_text(s.borrow().edit));
                let parsed = text.trim().replace(',', ".");
                if let Ok(v) = parsed.parse::<f32>() {
                    let v = v.clamp(0.01, 10.0);
                    SETTINGS.with(|s| s.borrow_mut().value = Some(v));
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
            SETTINGS.with(|s| s.borrow_mut().done = true);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub unsafe fn show_settings(owner: HWND, zoom_step: &mut f32) -> bool {
    let hinst = GetModuleHandleW(std::ptr::null());
    let dpi = GetDpiForWindow(owner);

    SETTINGS.with(|s| {
        let mut st = s.borrow_mut();
        st.done = false;
        st.value = None;
        st.initial = *zoom_step;
        st.edit = std::ptr::null_mut();
    });

    let w = scale(320, dpi);
    let h = scale(150, dpi);

    let mut rc: RECT = std::mem::zeroed();
    GetWindowRect(owner, &mut rc);
    let x = rc.left + ((rc.right - rc.left) - w) / 2;
    let y = rc.top + ((rc.bottom - rc.top) - h) / 2;

    let title = wide("Settings");
    let hwnd = CreateWindowExW(
        WS_EX_DLGMODALFRAME,
        wide(SETTINGS_CLASS).as_ptr(),
        title.as_ptr(),
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
    if hwnd.is_null() {
        return false;
    }

    let font = GetStockObject(DEFAULT_GUI_FONT);

    let label = CreateWindowExW(
        0,
        wide("STATIC").as_ptr(),
        wide("Zoom increment (0.01 - 10):").as_ptr(),
        WS_CHILD | WS_VISIBLE,
        scale(14, dpi),
        scale(18, dpi),
        scale(280, dpi),
        scale(20, dpi),
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let initial_text = format!("{}", *zoom_step);
    let edit = CreateWindowExW(
        WS_EX_CLIENTEDGE,
        wide("EDIT").as_ptr(),
        wide(&initial_text).as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | ES_AUTOHSCROLL as u32,
        scale(14, dpi),
        scale(44, dpi),
        scale(90, dpi),
        scale(24, dpi),
        hwnd,
        (100 as usize) as HMENU,
        hinst,
        std::ptr::null(),
    );

    let hint = CreateWindowExW(
        0,
        wide("STATIC").as_ptr(),
        wide("Saved to settings.txt next to the executable.").as_ptr(),
        WS_CHILD | WS_VISIBLE,
        scale(14, dpi),
        scale(74, dpi),
        scale(290, dpi),
        scale(20, dpi),
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let ok = CreateWindowExW(
        0,
        wide("BUTTON").as_ptr(),
        wide("OK").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
        scale(140, dpi),
        scale(100, dpi),
        scale(75, dpi),
        scale(26, dpi),
        hwnd,
        IDOK as HMENU,
        hinst,
        std::ptr::null(),
    );

    let cancel = CreateWindowExW(
        0,
        wide("BUTTON").as_ptr(),
        wide("Cancel").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
        scale(225, dpi),
        scale(100, dpi),
        scale(75, dpi),
        scale(26, dpi),
        hwnd,
        IDCANCEL as HMENU,
        hinst,
        std::ptr::null(),
    );

    for c in [label, edit, hint, ok, cancel] {
        SendMessageW(c, WM_SETFONT, font as usize, 1);
    }

    SETTINGS.with(|s| s.borrow_mut().edit = edit);

    EnableWindow(owner, 0);
    SetFocus(edit);
    SendMessageW(edit, EM_SETSEL, 0, -1);

    let mut msg: MSG = std::mem::zeroed();
    loop {
        let done = SETTINGS.with(|s| s.borrow().done);
        if done || IsWindow(hwnd) == 0 {
            break;
        }
        let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
        if r <= 0 {
            break;
        }
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    EnableWindow(owner, 1);
    SetActiveWindow(owner);

    let value = SETTINGS.with(|s| s.borrow().value);
    if let Some(v) = value {
        *zoom_step = v;
        true
    } else {
        false
    }
}

pub unsafe fn show_about(owner: HWND) {
    let text = format!(
        "Thumbcache Viewer {}\n\nExtracts thumbnail images from Windows thumbnail cache databases.\n\nRewritten in Rust.\n\nGNU General Public License v3",
        env!("CARGO_PKG_VERSION")
    );
    msgbox(owner, &text, MB_OK | MB_ICONINFORMATION);
}
