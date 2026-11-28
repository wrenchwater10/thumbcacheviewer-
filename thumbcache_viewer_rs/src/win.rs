// Thin Win32 glue: re-exports the bindings we use plus the few constants,
// structures and listview messages that windows-sys does not expose.

#![allow(dead_code, non_snake_case, non_camel_case_types)]

pub use windows_sys::Win32::Foundation::*;
pub use windows_sys::Win32::Graphics::Gdi::*;
pub use windows_sys::Win32::System::DataExchange::*;
pub use windows_sys::Win32::System::LibraryLoader::*;
pub use windows_sys::Win32::System::Memory::*;
pub use windows_sys::Win32::UI::Controls::Dialogs::*;
pub use windows_sys::Win32::UI::Controls::*;
pub use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub use windows_sys::Win32::UI::Shell::*;
pub use windows_sys::Win32::UI::WindowsAndMessaging::*;

use core::ffi::c_void;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

#[link(name = "user32")]
extern "system" {
    pub fn GetDpiForWindow(hwnd: HWND) -> u32;
}

pub const PROGRAM_CAPTION: &str = "Thumbcache Viewer";
pub const HOME_PAGE: &str = "https://thumbcacheviewer.github.io/";

pub const SPLITTER_WIDTH: i32 = 6;
pub const NUM_COLUMNS: usize = 11;

pub const COLOR_WINDOW_IDX: i32 = 5;
pub const COLOR_3DFACE_IDX: i32 = 15;
pub const COLOR_3DSHADOW_IDX: i32 = 16;
pub const COLOR_3DHIGHLIGHT_IDX: i32 = 20;
pub const COLOR_WINDOWTEXT_IDX: i32 = 8;
pub const COLOR_GRAYTEXT_IDX: i32 = 17;

pub const SPLITTER_CLASS: &str = "tcv_splitter";

// Clipboard formats (CF_DIB / CF_UNICODETEXT).
pub const CLIPBOARD_CF_DIB: u32 = 8;
pub const CLIPBOARD_CF_UNICODETEXT: u32 = 13;

// Virtual key codes used by the preview pane.
pub const VK_OEM_PLUS: u32 = 0xBB;
pub const VK_OEM_MINUS: u32 = 0xBD;
pub const VK_ADD: u32 = 0x6B;
pub const VK_SUBTRACT: u32 = 0x6D;
pub const VK_0: u32 = 0x30;
pub const VK_NUMPAD0: u32 = 0x60;

// Listview messages.
pub const LVM_FIRST: u32 = 0x1000;
pub const LVM_SETBKCOLOR: u32 = LVM_FIRST + 1;
pub const LVM_GETITEMCOUNT: u32 = LVM_FIRST + 4;
pub const LVM_DELETEITEM: u32 = LVM_FIRST + 8;
pub const LVM_DELETEALLITEMS: u32 = LVM_FIRST + 9;
pub const LVM_GETNEXTITEM: u32 = LVM_FIRST + 12;
pub const LVM_GETITEMRECT: u32 = LVM_FIRST + 14;
pub const LVM_ENSUREVISIBLE: u32 = LVM_FIRST + 19;
pub const LVM_SETCOLUMNWIDTH: u32 = LVM_FIRST + 30;
pub const LVM_GETCOLUMNWIDTH: u32 = LVM_FIRST + 29;
pub const LVM_UPDATE: u32 = LVM_FIRST + 42;
pub const LVM_SETITEMSTATE: u32 = LVM_FIRST + 43;
pub const LVM_GETSELECTEDCOUNT: u32 = LVM_FIRST + 50;
pub const LVM_SETEXTENDEDLISTVIEWSTYLE: u32 = LVM_FIRST + 54;
pub const LVM_INSERTITEMW: u32 = LVM_FIRST + 77;
pub const LVM_INSERTCOLUMNW: u32 = LVM_FIRST + 97;
pub const LVM_GETHEADER: u32 = LVM_FIRST + 31;
pub const LVM_GETSUBITEMRECT: u32 = LVM_FIRST + 56;
pub const LVIR_BOUNDS: i32 = 0;
pub const LVM_SETTEXTCOLOR: u32 = LVM_FIRST + 36;
pub const LVM_SETTEXTBKCOLOR: u32 = LVM_FIRST + 38;
pub const LVM_SETITEMTEXTW: u32 = LVM_FIRST + 116;

// Custom draw notifications and return values.
pub const NM_CUSTOMDRAW: u32 = 0xFFFF_FFF4;
pub const CDDS_PREPAINT: u32 = 0x0000_0001;
pub const CDDS_ITEMPREPAINT: u32 = 0x0001_0001;
pub const CDRF_DODEFAULT: isize = 0;
pub const CDRF_NEWFONT: isize = 0x0000_0002;
pub const CDRF_SKIPDEFAULT: isize = 0x0000_0004;
pub const CDRF_NOTIFYITEMDRAW: isize = 0x0000_0020;
pub const CDIS_SELECTED: u32 = 0x0000_0001;
pub const INDEX_WNDPROC: i32 = -4;
pub const INDEX_STYLE: i32 = -16;

// Static control styles (SS_LEFT is zero, so it is spelled out here).
pub const SS_TEXT_LEFT: u32 = 0x0000_0000;

pub const LVN_FIRST: u32 = 0u32.wrapping_sub(100);
pub const LVN_ITEMCHANGED: u32 = LVN_FIRST.wrapping_sub(1);
pub const LVN_COLUMNCLICK: u32 = LVN_FIRST.wrapping_sub(8);
pub const LVN_KEYDOWN: u32 = LVN_FIRST.wrapping_sub(55);

pub const LVIF_STATE: u32 = 0x0008;
pub const LVIF_TEXT: u32 = 0x0001;
pub const LVIF_PARAM: u32 = 0x0004;
pub const LVIS_FOCUSED: u32 = 0x0001;
pub const LVIS_SELECTED: u32 = 0x0002;
pub const LVNI_FOCUSED: u32 = 0x0001;
pub const LVNI_SELECTED: u32 = 0x0002;

pub const LVS_REPORT: u32 = 0x0001;
pub const LVS_SINGLESEL: u32 = 0x0004;
pub const LVS_SHOWSELALWAYS: u32 = 0x0008;
pub const LVS_EX_FULLROWSELECT: u32 = 0x00000020;
pub const LVS_EX_DOUBLEBUFFER: u32 = 0x00010000;
pub const LVS_EX_GRIDLINES: u32 = 0x00000001;

pub const WC_LISTVIEWW: &str = "SysListView32";

#[repr(C)]
pub struct LvColumn {
    pub mask: u32,
    pub fmt: i32,
    pub cx: i32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iSubItem: i32,
    pub iImage: i32,
    pub iOrder: i32,
    pub cxMin: i32,
    pub cxDefault: i32,
    pub cxIdeal: i32,
}

#[repr(C)]
pub struct LvItem {
    pub mask: u32,
    pub iItem: i32,
    pub iSubItem: i32,
    pub state: u32,
    pub stateMask: u32,
    pub pszText: *mut u16,
    pub cchTextMax: i32,
    pub iImage: i32,
    pub lParam: isize,
    pub iIndent: i32,
    pub iGroupId: i32,
    pub cColumns: u32,
    pub puColumns: *mut u32,
    pub piColFmt: *mut i32,
    pub iGroup: i32,
}

#[repr(C)]
pub struct Nmhdr {
    pub hwndFrom: HWND,
    pub idFrom: usize,
    pub code: u32,
}

#[repr(C)]
pub struct Nmlistview {
    pub hdr: Nmhdr,
    pub iItem: i32,
    pub iSubItem: i32,
    pub uNewState: u32,
    pub uOldState: u32,
    pub uChanged: u32,
    pub ptAction: POINT,
    pub lParam: isize,
}

#[repr(C)]
pub struct Nmcustomdraw {
    pub hdr: Nmhdr,
    pub dwDrawStage: u32,
    pub hdc: HDC,
    pub rc: RECT,
    pub dwItemSpec: usize,
    pub uItemState: u32,
    pub lItemlParam: isize,
}

#[repr(C)]
pub struct Nmlvcustomdraw {
    pub nmcd: Nmcustomdraw,
    pub clrText: u32,
    pub clrTextBk: u32,
    pub iSubItem: i32,
}

#[repr(C)]
pub struct Nmcolumnclick {
    pub hdr: Nmhdr,
    pub iItem: i32,
    pub iSubItem: i32,
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn wide_os(s: &OsStr) -> Vec<u16> {
    s.encode_wide().chain(std::iter::once(0)).collect()
}

pub unsafe fn from_wide(ptr: *const u16) -> String {
    if ptr.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

/// Message box that follows the dark theme. Falls back to the system box only
/// when there is no themed window yet (startup failures before the app exists).
pub unsafe fn msgbox(owner: HWND, text: &str, flags: u32) -> i32 {
    if crate::app::is_ready() {
        crate::dialogs::show_message(owner, text, (flags & MB_ICONWARNING) != 0);
        return 1;
    }
    let t = wide(text);
    let c = wide(PROGRAM_CAPTION);
    MessageBoxW(owner, t.as_ptr(), c.as_ptr(), flags)
}

pub fn scale(value: i32, dpi: u32) -> i32 {
    ((value as i64 * dpi as i64) / 96) as i32
}

pub fn null_mut<T>() -> *mut T {
    core::ptr::null_mut()
}

pub fn as_cvoid<T>(p: *const T) -> *const c_void {
    p as *const c_void
}
