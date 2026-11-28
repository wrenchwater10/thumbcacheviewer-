// Application state, main window behaviour, listview and commands.

#![allow(dead_code)]

use crate::chrome;
use crate::dialogs;
use crate::preview::{self, Preview};
use crate::save;
use crate::settings::{self, Settings};
use crate::theme;
use crate::thumbcache::{self, Entry};
use crate::win::*;
use std::cell::RefCell;
use std::ffi::OsString;
use std::path::PathBuf;

pub const MENU_OPEN: usize = 1001;
pub const MENU_SAVE_ALL: usize = 1002;
pub const MENU_SAVE_SEL: usize = 1003;
pub const MENU_EXPORT: usize = 1004;
pub const MENU_EXIT: usize = 1005;
pub const MENU_ABOUT: usize = 1006;
pub const MENU_SELECT_ALL: usize = 1007;
pub const MENU_REMOVE_SEL: usize = 1008;
pub const MENU_HIDE_BLANK: usize = 1009;
pub const MENU_CHECKSUMS: usize = 1010;
pub const MENU_SCAN: usize = 1011;
pub const MENU_INFO: usize = 1012;
pub const MENU_PROP_VALUE: usize = 1013;
pub const MENU_COPY_SEL: usize = 1014;
pub const MENU_HOME_PAGE: usize = 1015;
pub const MENU_COPY_IMAGE: usize = 1016;
pub const MENU_EXPORT_IMAGE: usize = 1017;
pub const MENU_SETTINGS: usize = 1018;
pub const MENU_ZOOM_IN: usize = 1020;
pub const MENU_ZOOM_OUT: usize = 1021;
pub const MENU_ZOOM_RESET: usize = 1022;

pub const COLUMNS: [&str; NUM_COLUMNS] = [
    "#",
    "Filename",
    "Cache Entry Offset",
    "Cache Entry Size",
    "Data Offset",
    "Data Size",
    "Data Checksum",
    "Header Checksum",
    "Cache Entry Hash",
    "System",
    "Location",
];

const COL_WIDTHS: [i32; NUM_COLUMNS] = [50, 260, 130, 110, 100, 90, 150, 150, 150, 120, 320];

pub struct App {
    pub hwnd: HWND,
    pub list: HWND,
    pub splitter: HWND,
    pub preview: Preview,
    pub entries: Vec<Entry>,
    pub visible: Vec<usize>,
    pub populating: bool,
    pub hide_blank: bool,
    pub show_checksums: bool,
    pub zoom_step: f32,
    pub dpi: u32,
    pub preview_width: i32,
    pub col_fmt: [u8; NUM_COLUMNS],
    pub toolbar: HWND,
    pub list_bar: HWND,
    pub preview_bar: HWND,
    pub status: HWND,
}

impl App {
    /// Refreshes the status bar from the current selection and zoom level.
    pub fn refresh_status(&self) {
        if self.status.is_null() {
            return;
        }
        let left = if self.preview.has_image() {
            format!(
                "{:.0}%     {} \u{00D7} {} px",
                self.preview.scale * 100.0,
                self.preview.width,
                self.preview.height
            )
        } else {
            String::from("No image")
        };
        let count = self.visible.len();
        let selected = unsafe { SendMessageW(self.list, LVM_GETSELECTEDCOUNT, 0, 0) } as usize;
        let mut right = if count == 1 {
            String::from("1 entry")
        } else {
            format!("{} entries", count)
        };
        if selected > 0 {
            right.push_str(&format!("     {} selected", selected));
        }
        let text = format!("{}\t{}", left, right);
        let mut wtext = wide(&text);
        unsafe {
            SetWindowTextW(self.status, wtext.as_mut_ptr());
        }
    }
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

pub fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|cell| {
        let mut borrow = cell.try_borrow_mut().ok()?;
        let app = borrow.as_mut()?;
        Some(f(app))
    })
}

pub fn is_ready() -> bool {
    APP.with(|cell| cell.borrow().is_some())
}

pub unsafe fn on_destroy() {
    persist_settings();
}

/// Writes the current settings to settings.txt beside the executable.
pub fn persist_settings() {
    with_app(|app| {
        settings::save(&Settings {
            zoom_step: app.zoom_step,
            hide_blank: app.hide_blank,
            show_checksums: app.show_checksums,
            preview_width: app.preview_width,
        });
    });
}

// ---------------------------------------------------------------- window setup

pub unsafe fn build_menu() -> HMENU {
    let menu = CreateMenu();
    let file = CreatePopupMenu();
    AppendMenuW(file, MF_STRING, MENU_OPEN, wide("&Open...\tCtrl+O").as_ptr());
    AppendMenuW(file, MF_STRING, MENU_SAVE_ALL, wide("Save &All...").as_ptr());
    AppendMenuW(file, MF_STRING, MENU_SAVE_SEL, wide("Save &Selected...").as_ptr());
    AppendMenuW(file, MF_STRING, MENU_EXPORT, wide("&Export List as CSV...").as_ptr());
    AppendMenuW(file, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(file, MF_STRING, MENU_EXIT, wide("E&xit").as_ptr());
    AppendMenuW(menu, MF_POPUP, file as usize, wide("&File").as_ptr());

    let edit = CreatePopupMenu();
    AppendMenuW(edit, MF_STRING, MENU_COPY_SEL, wide("&Copy Selected").as_ptr());
    AppendMenuW(edit, MF_STRING, MENU_SELECT_ALL, wide("Select &All").as_ptr());
    AppendMenuW(edit, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(edit, MF_STRING, MENU_REMOVE_SEL, wide("&Remove Selected").as_ptr());
    AppendMenuW(menu, MF_POPUP, edit as usize, wide("&Edit").as_ptr());

    let view = CreatePopupMenu();
    AppendMenuW(view, MF_STRING, MENU_HIDE_BLANK, wide("Hide &Blank Entries").as_ptr());
    AppendMenuW(
        view,
        MF_STRING,
        MENU_CHECKSUMS,
        wide("Show C&hecksum Columns").as_ptr(),
    );
    AppendMenuW(view, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(view, MF_STRING, MENU_ZOOM_IN, wide("Zoom &In\tCtrl++").as_ptr());
    AppendMenuW(view, MF_STRING, MENU_ZOOM_OUT, wide("Zoom &Out\tCtrl+-").as_ptr());
    AppendMenuW(view, MF_STRING, MENU_ZOOM_RESET, wide("&Reset Zoom\tCtrl+0").as_ptr());
    AppendMenuW(menu, MF_POPUP, view as usize, wide("&View").as_ptr());

    let tools = CreatePopupMenu();
    AppendMenuW(tools, MF_STRING, MENU_SETTINGS, wide("&Settings...").as_ptr());
    AppendMenuW(tools, MF_SEPARATOR, 0, std::ptr::null());
    AppendMenuW(tools, MF_STRING, MENU_HOME_PAGE, wide("&Home Page").as_ptr());
    AppendMenuW(menu, MF_POPUP, tools as usize, wide("&Tools").as_ptr());

    let help = CreatePopupMenu();
    AppendMenuW(help, MF_STRING, MENU_ABOUT, wide("&About").as_ptr());
    AppendMenuW(menu, MF_POPUP, help as usize, wide("&Help").as_ptr());

    menu
}

pub unsafe fn create(hwnd: HWND, cmd_line: &CommandLine) {
    let dpi = GetDpiForWindow(hwnd);
    let hinst = GetModuleHandleW(std::ptr::null());

    let list = CreateWindowExW(
        0,
        wide(WC_LISTVIEWW).as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | LVS_REPORT | LVS_SHOWSELALWAYS as u32,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    SendMessageW(
        list,
        LVM_SETEXTENDEDLISTVIEWSTYLE,
        (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as usize,
        (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as isize,
    );

    // Dark list: Explorer dark theme for its scrollbars, our own colours and a
    // custom drawn header and rows (so the system selection colour never shows).
    SendMessageW(list, LVM_SETBKCOLOR, 0, theme::PANEL as isize);
    SendMessageW(list, LVM_SETTEXTCOLOR, 0, theme::TEXT as isize);
    SendMessageW(list, LVM_SETTEXTBKCOLOR, 0, theme::PANEL as isize);
    theme::darken_control(list);
    theme::darken_scrollbars(list);
    let header = SendMessageW(list, LVM_GETHEADER, 0, 0) as HWND;
    if !header.is_null() {
        theme::untheme(header);
    }
    install_list_subclass(list);

    for (i, name) in COLUMNS.iter().enumerate() {
        let mut text = wide(name);
        let mut col = LvColumn {
            mask: LVCF_TEXT | LVCF_WIDTH | LVCF_SUBITEM,
            fmt: LVCFMT_LEFT,
            cx: scale(COL_WIDTHS[i], dpi),
            pszText: text.as_mut_ptr(),
            cchTextMax: 0,
            iSubItem: i as i32,
            iImage: 0,
            iOrder: 0,
            cxMin: 0,
            cxDefault: 0,
            cxIdeal: 0,
        };
        SendMessageW(list, LVM_INSERTCOLUMNW, i, &mut col as *mut _ as isize);
    }

    let splitter = CreateWindowExW(
        0,
        wide(SPLITTER_CLASS).as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let preview_hwnd = CreateWindowExW(
        0,
        wide(preview::PREVIEW_CLASS).as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let toolbar = CreateWindowExW(
        0,
        wide(chrome::TOOLBAR_CLASS).as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let list_bar = CreateWindowExW(
        0,
        wide(chrome::PANELBAR_CLASS).as_ptr(),
        wide("Cache entries").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let preview_bar = CreateWindowExW(
        0,
        wide(chrome::PANELBAR_CLASS).as_ptr(),
        wide("Preview").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    let status = CreateWindowExW(
        0,
        wide(chrome::STATUS_CLASS).as_ptr(),
        wide("").as_ptr(),
        WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
        0,
        0,
        0,
        0,
        hwnd,
        std::ptr::null_mut(),
        hinst,
        std::ptr::null(),
    );

    // Settings persist in settings.txt next to the executable.
    let saved = settings::load();

    let mut preview = Preview::new(preview_hwnd);
    preview.zoom_step = saved.zoom_step;

    let app = App {
        hwnd,
        list,
        splitter,
        preview,
        entries: Vec::new(),
        visible: Vec::new(),
        populating: false,
        hide_blank: cmd_line.hide_blank || saved.hide_blank,
        show_checksums: saved.show_checksums,
        zoom_step: saved.zoom_step,
        dpi,
        preview_width: saved.preview_width,
        col_fmt: [0; NUM_COLUMNS],
        toolbar,
        list_bar,
        preview_bar,
        status,
    };

    APP.with(|cell| *cell.borrow_mut() = Some(app));

    let menu = GetMenu(hwnd);
    CheckMenuItem(
        menu,
        MENU_HIDE_BLANK as u32,
        if cmd_line.hide_blank || saved.hide_blank {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        },
    );
    CheckMenuItem(
        menu,
        MENU_CHECKSUMS as u32,
        if saved.show_checksums {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        },
    );

    with_app(|app| app.refresh_status());
    layout(hwnd);
}

pub unsafe fn layout(hwnd: HWND) {
    let mut rc: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut rc);
    let cw = rc.right;
    let ch = rc.bottom;

    let geometry = with_app(|app| {
        let min_list = scale(160, app.dpi);
        let min_preview = scale(100, app.dpi);

        if app.preview_width < 0 {
            app.preview_width = (cw * 60) / 100;
        }

        let max_preview = (cw - min_list - SPLITTER_WIDTH).max(min_preview);
        if app.preview_width > max_preview {
            app.preview_width = max_preview;
        }
        if app.preview_width < min_preview {
            app.preview_width = min_preview.min(max_preview);
        }

        (
            app.list,
            app.splitter,
            app.preview.hwnd,
            app.preview_width,
            app.toolbar,
            app.list_bar,
            app.preview_bar,
            app.status,
            app.dpi,
        )
    });

    // The borrow is released before any window is moved: SetWindowPos sends
    // WM_SIZE synchronously and the panes handle it through with_app, which
    // cannot re-enter while the app is already borrowed.
    let Some((
        list,
        splitter,
        preview_hwnd,
        preview_w,
        toolbar,
        list_bar,
        preview_bar,
        status,
        dpi,
    )) = geometry
    else {
        return;
    };

    let tool_h = scale(chrome::TOOLBAR_HEIGHT, dpi);
    let bar_h = scale(chrome::PANELBAR_HEIGHT, dpi);
    let status_h = scale(chrome::STATUS_HEIGHT, dpi);
    let content_h = (ch - tool_h - status_h).max(0);
    let pane_h = (content_h - bar_h).max(0);
    let list_w = (cw - preview_w - SPLITTER_WIDTH).max(0);
    let preview_x = list_w + SPLITTER_WIDTH;
    let pane_y = tool_h + bar_h;

    // Move all three children in one transaction so that a resize never
    // draws a half-moved frame where the panes overlap.
    let flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS;
    let mut hdwp = BeginDeferWindowPos(7);
    if !hdwp.is_null() {
        hdwp = DeferWindowPos(hdwp, toolbar, std::ptr::null_mut(), 0, 0, cw, tool_h, flags);
        hdwp = DeferWindowPos(hdwp, list_bar, std::ptr::null_mut(), 0, tool_h, list_w, bar_h, flags);
        hdwp = DeferWindowPos(
            hdwp,
            list,
            std::ptr::null_mut(),
            0,
            pane_y,
            list_w,
            pane_h,
            flags,
        );
        hdwp = DeferWindowPos(
            hdwp,
            splitter,
            std::ptr::null_mut(),
            list_w,
            tool_h,
            SPLITTER_WIDTH,
            content_h,
            flags,
        );
        hdwp = DeferWindowPos(
            hdwp,
            preview_bar,
            std::ptr::null_mut(),
            preview_x,
            tool_h,
            preview_w,
            bar_h,
            flags,
        );
        hdwp = DeferWindowPos(
            hdwp,
            preview_hwnd,
            std::ptr::null_mut(),
            preview_x,
            pane_y,
            preview_w,
            pane_h,
            flags,
        );
        hdwp = DeferWindowPos(
            hdwp,
            status,
            std::ptr::null_mut(),
            0,
            ch - status_h,
            cw,
            status_h,
            flags,
        );
        EndDeferWindowPos(hdwp);
    } else {
        MoveWindow(toolbar, 0, 0, cw, tool_h, 0);
        MoveWindow(list_bar, 0, tool_h, list_w, bar_h, 0);
        MoveWindow(list, 0, pane_y, list_w, pane_h, 0);
        MoveWindow(splitter, list_w, tool_h, SPLITTER_WIDTH, content_h, 0);
        MoveWindow(preview_bar, preview_x, tool_h, preview_w, bar_h, 0);
        MoveWindow(preview_hwnd, preview_x, pane_y, preview_w, pane_h, 0);
        MoveWindow(status, 0, ch - status_h, cw, status_h, 0);
    }

    repaint_frame(
        hwnd,
        &[
            toolbar,
            list_bar,
            list,
            splitter,
            preview_bar,
            preview_hwnd,
            status,
        ],
    );

    // Scrollbars are created on demand, so re-theme them after each layout.
    theme::darken_scrollbars(list);
}

/// Repaints the frame and every pane synchronously, bottom of the z-order first.
///
/// Each window that owns pixels of the frame is invalidated in full and forced
/// to paint immediately for this layout, so a resize or splitter drag can never
/// leave pixels from an earlier frame on screen. Nothing is erased to the
/// background first: every pane paints its own background in one go (the list
/// is double-buffered, the preview blits a full back buffer), which keeps this
/// flicker free while still covering every pixel.
pub unsafe fn repaint_frame(hwnd: HWND, children: &[HWND]) {
    InvalidateRect(hwnd, std::ptr::null(), 0);
    UpdateWindow(hwnd);

    for child in children {
        if child.is_null() {
            continue;
        }
        InvalidateRect(*child, std::ptr::null(), 0);
        UpdateWindow(*child);
    }
}

pub unsafe extern "system" fn splitter_proc(
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
            theme::fill(hdc, &rc, theme::BG);
            theme::vline(hdc, 0, 0, rc.bottom, theme::BORDER);
            theme::vline(hdc, rc.right - 1, 0, rc.bottom, theme::BORDER);

            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            SetCapture(hwnd);
            SetFocus(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            let mut pt = POINT {
                x: (lparam as i32) as i16 as i32,
                y: ((lparam as i32) >> 16) as i16 as i32,
            };
            ClientToScreen(hwnd, &mut pt);
            if GetCapture() == hwnd {
                with_app(|app| {
                    let mut rc: RECT = std::mem::zeroed();
                    GetClientRect(app.hwnd, &mut rc);
                    let min_list = scale(160, app.dpi);
                    let min_preview = scale(100, app.dpi);
                    let mut new_preview = rc.right - pt.x;
                    if new_preview < min_preview {
                        new_preview = min_preview;
                    }
                    if new_preview > rc.right - min_list - SPLITTER_WIDTH {
                        new_preview = rc.right - min_list - SPLITTER_WIDTH;
                    }
                    app.preview_width = new_preview;
                });
                let main = with_app(|app| app.hwnd).unwrap_or(hwnd);
                layout(main);
            }
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            persist_settings();
            0
        }
        WM_SETCURSOR => {
            let cur = LoadCursorW(std::ptr::null_mut(), IDC_SIZEWE);
            SetCursor(cur);
            1
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ---------------------------------------------------------------- list handling

fn format_number(value: u32, mode: u8) -> String {
    match mode {
        1 => format!("{:X}", value),
        2 => format!("{:.2} KB", value as f64 / 1024.0),
        _ => value.to_string(),
    }
}

fn format_hex(value: u64, mode: u8) -> String {
    match mode {
        1 => format!("{:016x}", value),
        _ => format!("{:016X}", value),
    }
}

pub fn text_for(app: &App, col: usize, row: usize, entry: &Entry) -> String {
    match col {
        0 => (row + 1).to_string(),
        1 => entry.filename.clone(),
        2 => format_number(entry.header_offset, app.col_fmt[2]),
        3 => format_number(entry.cache_entry_size, app.col_fmt[3]),
        4 => format_number(entry.data_offset, app.col_fmt[4]),
        5 => format_number(entry.data_size, app.col_fmt[5]),
        6 => {
            if app.show_checksums {
                format_hex(entry.data_checksum, app.col_fmt[6])
            } else {
                String::new()
            }
        }
        7 => {
            if app.show_checksums {
                format_hex(entry.header_checksum, app.col_fmt[7])
            } else {
                String::new()
            }
        }
        8 => format_hex(entry.entry_hash, app.col_fmt[8]),
        9 => entry.system_name().to_string(),
        10 => entry.db_path.to_string_lossy().into_owned(),
        _ => String::new(),
    }
}

unsafe fn set_subitem(list: HWND, row: i32, col: i32, text: &str) {
    let mut wtext = wide(text);
    let mut item = LvItem {
        mask: LVIF_TEXT,
        iItem: row,
        iSubItem: col,
        state: 0,
        stateMask: 0,
        pszText: wtext.as_mut_ptr(),
        cchTextMax: wtext.len() as i32,
        iImage: 0,
        lParam: 0,
        iIndent: 0,
        iGroupId: 0,
        cColumns: 0,
        puColumns: std::ptr::null_mut(),
        piColFmt: std::ptr::null_mut(),
        iGroup: -1,
    };
    SendMessageW(list, LVM_SETITEMTEXTW, row as usize, &mut item as *mut _ as isize);
}

pub unsafe fn populate(hwnd: HWND) {
    let _ = hwnd;
    with_app(|app| {
        let list = app.list;
        app.populating = true;
        SendMessageW(list, LVM_DELETEALLITEMS, 0, 0);

        app.visible.clear();
        for (i, e) in app.entries.iter().enumerate() {
            if app.hide_blank && e.data_size == 0 {
                continue;
            }
            app.visible.push(i);
        }

        for (row, &idx) in app.visible.iter().enumerate() {
            let e = &app.entries[idx];
            let mut first = wide(&text_for(app, 0, row, e));
            let mut item = LvItem {
                mask: LVIF_PARAM | LVIF_TEXT,
                iItem: row as i32,
                iSubItem: 0,
                state: 0,
                stateMask: 0,
                pszText: first.as_mut_ptr(),
                cchTextMax: first.len() as i32,
                iImage: 0,
                lParam: idx as isize,
                iIndent: 0,
                iGroupId: 0,
                cColumns: 0,
                puColumns: std::ptr::null_mut(),
                piColFmt: std::ptr::null_mut(),
                iGroup: -1,
            };
            SendMessageW(list, LVM_INSERTITEMW, 0, &mut item as *mut _ as isize);
            for col in 1..NUM_COLUMNS {
                set_subitem(list, row as i32, col as i32, &text_for(app, col, row, e));
            }
        }

        // Nothing may start out tinted: clear every selected and focused flag
        // before the caller picks the row it actually wants.
        let mut clear = LvItem {
            mask: LVIF_STATE,
            iItem: -1,
            iSubItem: 0,
            state: 0,
            stateMask: LVIS_SELECTED | LVIS_FOCUSED,
            pszText: std::ptr::null_mut(),
            cchTextMax: 0,
            iImage: 0,
            lParam: 0,
            iIndent: 0,
            iGroupId: 0,
            cColumns: 0,
            puColumns: std::ptr::null_mut(),
            piColFmt: std::ptr::null_mut(),
            iGroup: -1,
        };
        SendMessageW(
            list,
            LVM_SETITEMSTATE,
            usize::MAX,
            &mut clear as *mut LvItem as isize,
        );

        app.populating = false;
        app.refresh_status();
        theme::darken_scrollbars(list);
    });
}

unsafe fn refresh_rows() {
    with_app(|app| {
        let list = app.list;
        for (row, &idx) in app.visible.iter().enumerate() {
            for col in 0..NUM_COLUMNS {
                set_subitem(list, row as i32, col as i32, &text_for(app, col, row, &app.entries[idx]));
            }
        }
        InvalidateRect(list, std::ptr::null(), 0);
    });
}

unsafe fn select_row(row: i32) {
    with_app(|app| {
        let mut item = LvItem {
            mask: LVIF_STATE,
            iItem: row,
            iSubItem: 0,
            state: LVIS_SELECTED | LVIS_FOCUSED,
            stateMask: LVIS_SELECTED | LVIS_FOCUSED,
            pszText: std::ptr::null_mut(),
            cchTextMax: 0,
            iImage: 0,
            lParam: 0,
            iIndent: 0,
            iGroupId: 0,
            cColumns: 0,
            puColumns: std::ptr::null_mut(),
            piColFmt: std::ptr::null_mut(),
            iGroup: -1,
        };
        SendMessageW(
            app.list,
            LVM_SETITEMSTATE,
            row as usize,
            &mut item as *mut _ as isize,
        );
        SendMessageW(app.list, LVM_ENSUREVISIBLE, row as usize, 0);
    });
}

unsafe fn show_row(row: i32) {
    let entry = with_app(|app| {
        if row < 0 || row as usize >= app.visible.len() {
            return None;
        }
        Some(app.entries[app.visible[row as usize]].clone())
    })
    .flatten();

    let Some(entry) = entry else {
        return;
    };

    if entry.data_size == 0 {
        with_app(|app| {
            app.preview.set_message("This cache entry has no image data.");
            app.refresh_status();
        });
        return;
    }

    let bytes = match entry.data() {
        Ok(b) => b,
        Err(e) => {
            with_app(|app| {
                app.preview.set_message(&format!("Unable to read data: {}", e));
                app.refresh_status();
            });
            return;
        }
    };

    match image::load_from_memory(&bytes) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            let pixels = rgba.into_raw();
            with_app(|app| {
                app.preview.set_image(w, h, pixels);
                app.refresh_status();
            });
        }
        Err(e) => {
            with_app(|app| {
                app.preview
                    .set_message(&format!("Unable to decode image: {}", e));
                app.refresh_status();
            });
        }
    }
}

pub unsafe fn on_notify(lp: LPARAM) -> LRESULT {
    let hdr = &*(lp as *const Nmhdr);
    match hdr.code {
        LVN_ITEMCHANGED => {
            let nm = &*(lp as *const Nmlistview);
            let populating = with_app(|app| app.populating).unwrap_or(true);
            if !populating
                && (nm.uNewState & LVIS_FOCUSED) != 0
                && (nm.uOldState & LVIS_FOCUSED) == 0
            {
                show_row(nm.iItem);
            }
            if !populating && (nm.uChanged & LVIF_STATE) != 0 {
                with_app(|app| app.refresh_status());
            }
            0
        }
        LVN_COLUMNCLICK => {
            let nm = &*(lp as *const Nmcolumnclick);
            let col = nm.iSubItem as usize;
            if (2..=5).contains(&col) {
                with_app(|app| app.col_fmt[col] = (app.col_fmt[col] + 1) % 3);
            } else if (6..=8).contains(&col) {
                with_app(|app| app.col_fmt[col] = (app.col_fmt[col] + 1) % 2);
            }
            refresh_rows();
            0
        }
        NM_CUSTOMDRAW => {
            let draw = &mut *(lp as *mut Nmlvcustomdraw);
            list_custom_draw(draw)
        }
        _ => 0,
    }
}

/// Custom draws the dark list rows: dark background, light text and an accent
/// coloured selection instead of the system highlight.
fn list_custom_draw(draw: &mut Nmlvcustomdraw) -> LRESULT {
    match draw.nmcd.dwDrawStage {
        CDDS_PREPAINT => {
            draw.clrText = theme::TEXT;
            draw.clrTextBk = theme::PANEL;
            CDRF_NOTIFYITEMDRAW
        }
        CDDS_ITEMPREPAINT => unsafe { draw_list_row(draw) },
        _ => CDRF_DODEFAULT,
    }
}

/// Paints a whole list row by hand: background, every cell's text and the
/// selection colour. The control's own item painting is skipped, which is what
/// keeps the blue system highlight from appearing on rows nobody selected.
unsafe fn draw_list_row(draw: &mut Nmlvcustomdraw) -> LRESULT {
    let list = draw.nmcd.hdr.hwndFrom;
    let hdc = draw.nmcd.hdc;
    let row = draw.nmcd.dwItemSpec as i32;
    let selected = (draw.nmcd.uItemState & CDIS_SELECTED) != 0;
    let dpi = GetDpiForWindow(list);

    let mut client: RECT = std::mem::zeroed();
    GetClientRect(list, &mut client);

    let mut bounds = RECT {
        left: LVIR_BOUNDS,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if SendMessageW(
        list,
        LVM_GETITEMRECT,
        row as usize,
        &mut bounds as *mut RECT as isize,
    ) == 0
    {
        return CDRF_SKIPDEFAULT;
    }
    bounds.left = 0;
    bounds.right = client.right;
    theme::fill(
        hdc,
        &bounds,
        if selected {
            theme::SELECT
        } else {
            theme::PANEL
        },
    );

    let cells: Vec<String> = with_app(|app| {
        if row < 0 || row as usize >= app.visible.len() {
            return Vec::new();
        }
        let index = app.visible[row as usize];
        (0..NUM_COLUMNS)
            .map(|col| text_for(app, col, row as usize, &app.entries[index]))
            .collect()
    })
    .unwrap_or_default();

    let color = if selected {
        theme::TEXT_BRIGHT
    } else {
        theme::TEXT
    };

    for col in 0..NUM_COLUMNS {
        let Some(text) = cells.get(col) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        let mut cell = RECT {
            left: LVIR_BOUNDS,
            top: col as i32,
            right: 0,
            bottom: 0,
        };
        if SendMessageW(
            list,
            LVM_GETSUBITEMRECT,
            row as usize,
            &mut cell as *mut RECT as isize,
        ) == 0
        {
            continue;
        }
        cell.left += scale(6, dpi);
        cell.right -= scale(4, dpi);
        theme::text_with_font(
            hdc,
            text,
            cell,
            color,
            dpi,
            9,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }

    CDRF_SKIPDEFAULT
}

// ------------------------------------------------------------- list subclass

static mut OLD_LIST_PROC: WNDPROC = None;

/// The column header sends its custom draw notification to the listview, which
/// does not forward it, so the listview is subclassed to paint the header dark.
unsafe extern "system" fn list_subclass(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NOTIFY {
        let hdr = &*(lparam as *const Nmhdr);
        if hdr.code == NM_CUSTOMDRAW {
            let draw = &mut *(lparam as *mut Nmcustomdraw);
            match draw.dwDrawStage {
                CDDS_PREPAINT => {
                    theme::fill(draw.hdc, &draw.rc, theme::HEADER);
                    return CDRF_NOTIFYITEMDRAW;
                }
                CDDS_ITEMPREPAINT => {
                    header_item(draw);
                    return CDRF_SKIPDEFAULT;
                }
                _ => {}
            }
        }
    }
    CallWindowProcW(OLD_LIST_PROC, hwnd, msg, wparam, lparam)
}

/// The header has no coloured-text custom draw fields, so each column title is
/// painted here instead: dark cell, light label and a hairline separator.
unsafe fn header_item(draw: &Nmcustomdraw) {
    theme::fill(draw.hdc, &draw.rc, theme::HEADER);

    let dpi = GetDpiForWindow(draw.hdr.hwndFrom);
    if let Some(title) = COLUMNS.get(draw.dwItemSpec) {
        let mut rc = draw.rc;
        rc.left += scale(8, dpi);
        rc.right -= scale(6, dpi);
        theme::text_with_font(
            draw.hdc,
            title,
            rc,
            theme::TEXT,
            dpi,
            9,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
    }
    theme::vline(
        draw.hdc,
        draw.rc.right - 1,
        draw.rc.top,
        draw.rc.bottom,
        theme::BORDER,
    );
}

unsafe fn install_list_subclass(list: HWND) {
    let previous = GetWindowLongPtrW(list, INDEX_WNDPROC);
    OLD_LIST_PROC = std::mem::transmute::<isize, WNDPROC>(previous);
    SetWindowLongPtrW(
        list,
        INDEX_WNDPROC,
        list_subclass as *const () as usize as isize,
    );
}

// ---------------------------------------------------------------- file handling

pub unsafe fn open_files(hwnd: HWND, paths: &[PathBuf]) {
    if paths.is_empty() {
        return;
    }

    let cursor = LoadCursorW(std::ptr::null_mut(), IDC_WAIT);
    SetCursor(cursor);

    let mut added = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for path in paths {
        match thumbcache::read_database(path, false) {
            Ok(mut entries) => {
                added += entries.len();
                with_app(|app| app.entries.append(&mut entries));
            }
            Err(e) => errors.push(e),
        }
    }

    populate(hwnd);
    select_row(0);
    show_row(0);

    let arrow = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
    SetCursor(arrow);

    if !errors.is_empty() {
        let text = errors.join("\n");
        msgbox(hwnd, &text, MB_APPLMODAL | MB_ICONWARNING);
    } else if added == 0 {
        msgbox(hwnd, "No cache entries were found.", MB_APPLMODAL | MB_ICONINFORMATION);
    }
}

unsafe fn selected_entries() -> Vec<Entry> {
    with_app(|app| {
        let mut out = Vec::new();
        let mut idx = SendMessageW(app.list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize);
        while idx >= 0 {
            let row = idx as usize;
            if let Some(&entry_idx) = app.visible.get(row) {
                out.push(app.entries[entry_idx].clone());
            }
            idx = SendMessageW(app.list, LVM_GETNEXTITEM, idx as usize, LVNI_SELECTED as isize);
        }
        out
    })
    .unwrap_or_default()
}

pub unsafe fn on_command(hwnd: HWND, id: usize) {
    match id {
        MENU_OPEN => {
            let files = dialogs::open_files(hwnd);
            open_files(hwnd, &files);
        }
        MENU_SAVE_ALL => {
            let entries = with_app(|app| app.entries.clone()).unwrap_or_default();
            if entries.is_empty() {
                return;
            }
            if let Some(dir) = dialogs::pick_folder(hwnd) {
                let cursor = LoadCursorW(std::ptr::null_mut(), IDC_WAIT);
                SetCursor(cursor);
                let result = save::save_entries(&entries, &dir);
                let arrow = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
                SetCursor(arrow);
                match result {
                    Ok(n) => msgbox(
                        hwnd,
                        &format!("Saved {} image(s) to:\n{}", n, dir.display()),
                        MB_APPLMODAL | MB_ICONINFORMATION,
                    ),
                    Err(e) => msgbox(hwnd, &e, MB_APPLMODAL | MB_ICONWARNING),
                };
            }
        }
        MENU_SAVE_SEL => {
            let entries = selected_entries();
            if entries.is_empty() {
                msgbox(hwnd, "No entries are selected.", MB_APPLMODAL | MB_ICONINFORMATION);
                return;
            }
            if let Some(dir) = dialogs::pick_folder(hwnd) {
                match save::save_entries(&entries, &dir) {
                    Ok(n) => msgbox(
                        hwnd,
                        &format!("Saved {} image(s) to:\n{}", n, dir.display()),
                        MB_APPLMODAL | MB_ICONINFORMATION,
                    ),
                    Err(e) => msgbox(hwnd, &e, MB_APPLMODAL | MB_ICONWARNING),
                };
            }
        }
        MENU_EXPORT => {
            let entries = with_app(|app| app.entries.clone()).unwrap_or_default();
            if entries.is_empty() {
                return;
            }
            if let Some(path) = dialogs::save_file(
                hwnd,
                "Export List as CSV",
                &[("Comma Separated Values", "*.csv"), ("All Files", "*.*")],
                "csv",
                "thumbcache_entries.csv",
            ) {
                match save::save_csv(&path, &entries) {
                    Ok(n) => msgbox(
                        hwnd,
                        &format!("Exported {} entries to:\n{}", n, path.display()),
                        MB_APPLMODAL | MB_ICONINFORMATION,
                    ),
                    Err(e) => msgbox(hwnd, &e, MB_APPLMODAL | MB_ICONWARNING),
                };
            }
        }
        MENU_COPY_SEL => {
            let entries = selected_entries();
            if entries.is_empty() {
                return;
            }
            let mut text = String::new();
            text.push_str(&COLUMNS.join("\t"));
            text.push_str("\r\n");
            for (i, e) in entries.iter().enumerate() {
                let vals = [
                    (i + 1).to_string(),
                    e.filename.clone(),
                    e.header_offset.to_string(),
                    e.cache_entry_size.to_string(),
                    e.data_offset.to_string(),
                    e.data_size.to_string(),
                    format!("{:016X}", e.data_checksum),
                    format!("{:016X}", e.header_checksum),
                    format!("{:016X}", e.entry_hash),
                    e.system_name().to_string(),
                    e.db_path.to_string_lossy().into_owned(),
                ];
                text.push_str(&vals.join("\t"));
                text.push_str("\r\n");
            }
            save::copy_text_to_clipboard(hwnd, &text);
        }
        MENU_SELECT_ALL => {
            with_app(|app| {
                let count = SendMessageW(app.list, LVM_GETITEMCOUNT, 0, 0) as i32;
                for row in 0..count {
                    let mut item = LvItem {
                        mask: LVIF_STATE,
                        iItem: row,
                        iSubItem: 0,
                        state: LVIS_SELECTED,
                        stateMask: LVIS_SELECTED,
                        pszText: std::ptr::null_mut(),
                        cchTextMax: 0,
                        iImage: 0,
                        lParam: 0,
                        iIndent: 0,
                        iGroupId: 0,
                        cColumns: 0,
                        puColumns: std::ptr::null_mut(),
                        piColFmt: std::ptr::null_mut(),
                        iGroup: -1,
                    };
                    SendMessageW(
                        app.list,
                        LVM_SETITEMSTATE,
                        row as usize,
                        &mut item as *mut _ as isize,
                    );
                }
            });
        }
        MENU_REMOVE_SEL => {
            let keep: Vec<Entry> = with_app(|app| {
                let mut remove = std::collections::HashSet::new();
                let mut idx =
                    SendMessageW(app.list, LVM_GETNEXTITEM, usize::MAX, LVNI_SELECTED as isize);
                while idx >= 0 {
                    if let Some(&entry_idx) = app.visible.get(idx as usize) {
                        remove.insert(entry_idx);
                    }
                    idx = SendMessageW(
                        app.list,
                        LVM_GETNEXTITEM,
                        idx as usize,
                        LVNI_SELECTED as isize,
                    );
                }
                app.entries
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| !remove.contains(i))
                    .map(|(_, e)| e.clone())
                    .collect()
            })
            .unwrap_or_default();

            with_app(|app| app.entries = keep);
            populate(hwnd);
            show_row(0);
        }
        MENU_HIDE_BLANK => {
            with_app(|app| app.hide_blank = !app.hide_blank);
            let checked = with_app(|app| app.hide_blank).unwrap_or(false);
            CheckMenuItem(
                GetMenu(hwnd),
                MENU_HIDE_BLANK as u32,
                if checked { MF_CHECKED } else { MF_UNCHECKED },
            );
            populate(hwnd);
            show_row(0);
            persist_settings();
        }
        MENU_CHECKSUMS => {
            with_app(|app| app.show_checksums = !app.show_checksums);
            let checked = with_app(|app| app.show_checksums).unwrap_or(false);
            CheckMenuItem(
                GetMenu(hwnd),
                MENU_CHECKSUMS as u32,
                if checked { MF_CHECKED } else { MF_UNCHECKED },
            );
            refresh_rows();
            persist_settings();
        }
        MENU_ZOOM_IN => {
            with_app(|app| {
                app.preview.zoom(1.0);
                app.refresh_status();
            });
        }
        MENU_ZOOM_OUT => {
            with_app(|app| {
                app.preview.zoom(-1.0);
                app.refresh_status();
            });
        }
        MENU_ZOOM_RESET => {
            with_app(|app| {
                app.preview.reset_zoom();
                app.refresh_status();
            });
        }
        MENU_COPY_IMAGE => {
            let failure = with_app(|app| app.preview.copy_to_clipboard(app.hwnd)).flatten();
            if let Some(message) = failure {
                msgbox(hwnd, message, MB_OK | MB_ICONINFORMATION);
            }
        }
        MENU_EXPORT_IMAGE => {
            let path = dialogs::save_file(
                hwnd,
                "Export Image",
                &[
                    ("PNG Image", "*.png"),
                    ("JPEG Image", "*.jpg"),
                    ("Bitmap", "*.bmp"),
                ],
                "png",
                "thumbnail.png",
            );
            if let Some(path) = path {
                let result = with_app(|app| app.preview.export_to(&path)).flatten();
                match result {
                    Some(Ok(())) => {}
                    Some(Err(e)) => {
                        msgbox(hwnd, &e, MB_APPLMODAL | MB_ICONWARNING);
                    }
                    None => {}
                }
            }
        }
        MENU_SETTINGS => {
            let mut step = with_app(|app| app.zoom_step).unwrap_or(0.5);
            if dialogs::show_settings(hwnd, &mut step) {
                with_app(|app| {
                    app.zoom_step = step;
                    app.preview.zoom_step = step;
                });
                persist_settings();
            }
        }
        MENU_ABOUT => dialogs::show_about(hwnd),
        MENU_HOME_PAGE => {
            let url = wide(HOME_PAGE);
            ShellExecuteW(
                hwnd,
                wide("open").as_ptr(),
                url.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                SW_SHOWNORMAL,
            );
        }
        MENU_EXIT => {
            DestroyWindow(hwnd);
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- command line

#[derive(Default)]
pub struct CommandLine {
    pub paths: Vec<PathBuf>,
    pub output: Option<PathBuf>,
    pub csv: Option<PathBuf>,
    pub hide_blank: bool,
}

pub fn parse_args(args: &[OsString]) -> CommandLine {
    let mut cl = CommandLine::default();
    let mut i = 0usize;
    while i < args.len() {
        let arg = args[i].to_string_lossy().into_owned();
        let lower = arg.to_lowercase();
        if lower == "-o" || lower == "--output" {
            if i + 1 < args.len() {
                i += 1;
                cl.output = Some(PathBuf::from(&args[i]));
            }
        } else if lower == "-c" || lower == "--csv" {
            if i + 1 < args.len() {
                i += 1;
                cl.csv = Some(PathBuf::from(&args[i]));
            }
        } else if lower == "-z" || lower == "--hide-blank" {
            cl.hide_blank = true;
        } else if !arg.starts_with('-') {
            let p = PathBuf::from(&args[i]);
            cl.paths.push(std::fs::canonicalize(&p).unwrap_or(p));
        }
        i += 1;
    }
    cl
}

/// Non-interactive mode: -o <dir> saves images, -c <file.csv> writes the list.
pub fn run_headless(cl: &CommandLine) -> i32 {
    if cl.paths.is_empty() {
        eprintln!("No thumbcache database was given.");
        return 1;
    }

    let mut entries: Vec<Entry> = Vec::new();
    for path in &cl.paths {
        match thumbcache::read_database(path, false) {
            Ok(mut e) => entries.append(&mut e),
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        }
    }

    let filtered: Vec<Entry> = entries
        .into_iter()
        .filter(|e| !(cl.hide_blank && e.data_size == 0))
        .collect();

    if let Some(dir) = &cl.output {
        match save::save_entries(&filtered, dir) {
            Ok(n) => println!("Saved {} image(s) to {}", n, dir.display()),
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        }
    }

    if let Some(csv) = &cl.csv {
        match save::save_csv(csv, &filtered) {
            Ok(n) => println!("Exported {} entries to {}", n, csv.display()),
            Err(e) => {
                eprintln!("{}", e);
                return 1;
            }
        }
    }

    0
}
