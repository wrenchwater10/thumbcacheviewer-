// Application state, main window behaviour, listview and commands.

#![allow(dead_code)]

use crate::chrome;
use crate::grid;
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

pub const MENU_TITLES: [&str; 5] = ["File", "Edit", "View", "Tools", "Help"];

pub struct MenuEntry {
    pub label: &'static str,
    pub accel: &'static str,
    pub id: usize,
    pub separator: bool,
    pub checked: bool,
}

fn menu_item(label: &'static str, accel: &'static str, id: usize) -> MenuEntry {
    MenuEntry {
        label,
        accel,
        id,
        separator: false,
        checked: false,
    }
}

fn menu_separator() -> MenuEntry {
    MenuEntry {
        label: "",
        accel: "",
        id: 0,
        separator: true,
        checked: false,
    }
}

/// Entries of one of the custom popup menus (0 = File ... 4 = Help).
pub fn menu_entries(index: usize) -> Vec<MenuEntry> {
    let (hide_blank, checksums) =
        with_app(|app| (app.hide_blank, app.show_checksums)).unwrap_or((false, true));
    match index {
        0 => vec![
            menu_item("Open...", "Ctrl+O", MENU_OPEN),
            menu_item("Save All...", "", MENU_SAVE_ALL),
            menu_item("Save Selected...", "", MENU_SAVE_SEL),
            menu_item("Export List as CSV...", "", MENU_EXPORT),
            menu_separator(),
            menu_item("Exit", "", MENU_EXIT),
        ],
        1 => vec![
            menu_item("Copy Selected", "", MENU_COPY_SEL),
            menu_item("Select All", "", MENU_SELECT_ALL),
            menu_separator(),
            menu_item("Remove Selected", "", MENU_REMOVE_SEL),
        ],
        2 => vec![
            MenuEntry {
                checked: hide_blank,
                ..menu_item("Hide Blank Entries", "", MENU_HIDE_BLANK)
            },
            MenuEntry {
                checked: checksums,
                ..menu_item("Show Checksum Columns", "", MENU_CHECKSUMS)
            },
            menu_separator(),
            menu_item("Zoom In", "Ctrl++", MENU_ZOOM_IN),
            menu_item("Zoom Out", "Ctrl+-", MENU_ZOOM_OUT),
            menu_item("Fit to Window", "Ctrl+0", MENU_ZOOM_RESET),
        ],
        3 => vec![
            menu_item("Copy Image", "", MENU_COPY_IMAGE),
            menu_item("Export Image...", "", MENU_EXPORT_IMAGE),
            menu_separator(),
            menu_item("Settings...", "", MENU_SETTINGS),
            menu_item("Home Page", "", MENU_HOME_PAGE),
        ],
        _ => vec![menu_item("About", "", MENU_ABOUT)],
    }
}

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
    pub grid: HWND,
    pub top: HWND,
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
        let selected = grid::selected_count();
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

    let grid_hwnd = CreateWindowExW(
        0,
        wide(grid::GRID_CLASS).as_ptr(),
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

    let mut col_widths = [0i32; NUM_COLUMNS];
    for (i, width) in COL_WIDTHS.iter().enumerate() {
        col_widths[i] = scale(*width, dpi);
    }
    grid::init(grid_hwnd, col_widths);

    let top = CreateWindowExW(
        0,
        wide(chrome::TOP_CLASS).as_ptr(),
        wide(PROGRAM_CAPTION).as_ptr(),
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
        grid: grid_hwnd,
        top,
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
            app.preview_width = (cw * 62) / 100;
        }
        let max_preview = (cw - min_list - SPLITTER_WIDTH).max(min_preview);
        if app.preview_width > max_preview {
            app.preview_width = max_preview;
        }
        if app.preview_width < min_preview {
            app.preview_width = min_preview.min(max_preview);
        }

        (
            app.grid,
            app.splitter,
            app.preview.hwnd,
            app.preview_width,
            app.top,
            app.list_bar,
            app.preview_bar,
            app.status,
            app.dpi,
        )
    });

    let Some((
        grid_hwnd,
        splitter,
        preview_hwnd,
        preview_w,
        top,
        list_bar,
        preview_bar,
        status,
        dpi,
    )) = geometry
    else {
        return;
    };

    let top_h = chrome::top_height(dpi);
    let bar_h = scale(chrome::PANELBAR_H, dpi);
    let status_h = scale(chrome::STATUS_H, dpi);
    let content_h = (ch - top_h - status_h).max(0);
    let pane_h = (content_h - bar_h).max(0);
    let list_w = (cw - preview_w - SPLITTER_WIDTH).max(0);
    let preview_x = list_w + SPLITTER_WIDTH;
    let pane_y = top_h + bar_h;

    let flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOCOPYBITS;
    let mut hdwp = BeginDeferWindowPos(7);
    if !hdwp.is_null() {
        hdwp = DeferWindowPos(hdwp, top, std::ptr::null_mut(), 0, 0, cw, top_h, flags);
        hdwp = DeferWindowPos(hdwp, list_bar, std::ptr::null_mut(), 0, top_h, list_w, bar_h, flags);
        hdwp = DeferWindowPos(
            hdwp,
            grid_hwnd,
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
            top_h,
            SPLITTER_WIDTH,
            content_h,
            flags,
        );
        hdwp = DeferWindowPos(
            hdwp,
            preview_bar,
            std::ptr::null_mut(),
            preview_x,
            top_h,
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
        MoveWindow(top, 0, 0, cw, top_h, 0);
        MoveWindow(list_bar, 0, top_h, list_w, bar_h, 0);
        MoveWindow(grid_hwnd, 0, pane_y, list_w, pane_h, 0);
        MoveWindow(splitter, list_w, top_h, SPLITTER_WIDTH, content_h, 0);
        MoveWindow(preview_bar, preview_x, top_h, preview_w, bar_h, 0);
        MoveWindow(preview_hwnd, preview_x, pane_y, preview_w, pane_h, 0);
        MoveWindow(status, 0, ch - status_h, cw, status_h, 0);
    }

    repaint_frame(
        hwnd,
        &[
            top,
            list_bar,
            grid_hwnd,
            splitter,
            preview_bar,
            preview_hwnd,
            status,
        ],
    );
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
        app.visible.clear();
        for (i, entry) in app.entries.iter().enumerate() {
            if app.hide_blank && entry.data_size == 0 {
                continue;
            }
            app.visible.push(i);
        }
        app.refresh_status();
    });
    let count = with_app(|app| app.visible.len()).unwrap_or(0);
    grid::set_count(count);
}

unsafe fn refresh_rows() {
    grid::invalidate();
}

unsafe fn select_row(row: i32) {
    if row < 0 {
        return;
    }
    grid::set_selection(&[row as usize]);
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

pub fn on_selection_changed() {
    let first = grid::with_grid(|grid| grid.first_selected()).flatten();
    unsafe {
        if let Some(row) = first {
            show_row(row as i32);
        }
        with_app(|app| app.refresh_status());
    }
}

pub unsafe fn cycle_column_format(col: usize) {
    with_app(|app| {
        if (2..=5).contains(&col) {
            app.col_fmt[col] = (app.col_fmt[col] + 1) % 3;
        } else if (6..=8).contains(&col) {
            app.col_fmt[col] = (app.col_fmt[col] + 1) % 2;
        }
    });
    grid::invalidate();
}

/// Text for every column of a row range, in one app borrow.
pub fn row_texts(app: &App, first: usize, last: usize) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    for row in first..last.min(app.visible.len()) {
        let index = app.visible[row];
        let entry = &app.entries[index];
        rows.push(
            (0..NUM_COLUMNS)
                .map(|col| text_for(app, col, row, entry))
                .collect(),
        );
    }
    rows
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
    let rows = grid::selected_rows();
    with_app(|app| {
        rows.iter()
            .filter_map(|&row| {
                app.visible
                    .get(row)
                    .map(|&index| app.entries[index].clone())
            })
            .collect()
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
            grid::select_all();
        }
        MENU_REMOVE_SEL => {
            let rows = grid::selected_rows();
            let keep: Vec<Entry> = with_app(|app| {
                let mut remove = std::collections::HashSet::new();
                for row in &rows {
                    if let Some(&index) = app.visible.get(*row) {
                        remove.insert(index);
                    }
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
            select_row(0);
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
