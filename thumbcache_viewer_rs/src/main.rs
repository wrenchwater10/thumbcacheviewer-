// Thumbcache Viewer - Rust/Win32 rewrite.
//
// Entry point, window class registration, the main window procedure and the
// message loop.

#![windows_subsystem = "windows"]

mod app;
mod chrome;
mod dialogs;
mod preview;
mod save;
mod settings;
mod theme;
mod thumbcache;
mod win;

use crate::win::*;
use std::ffi::OsString;
use std::path::PathBuf;

const MAIN_CLASS: &str = "thumbcache_viewer_rust";

#[link(name = "kernel32")]
extern "system" {
    fn AttachConsole(process_id: u32) -> i32;
}

#[repr(C)]
struct Accel {
    f_virt: u8,
    key: u16,
    cmd: u16,
}

const FVIRTKEY: u8 = 0x01;
const FCONTROL: u8 = 0x08;

unsafe extern "system" fn main_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_SIZE => {
            app::layout(hwnd);
            0
        }
        WM_ERASEBKGND => {
            let hdc: HDC = wparam as *mut core::ffi::c_void;
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            theme::fill(hdc, &rc, theme::BG);
            1
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            theme::fill(hdc, &ps.rcPaint, theme::BG);
            EndPaint(hwnd, &ps);
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            app::on_command(hwnd, id);
            0
        }
        WM_NOTIFY => app::on_notify(lparam),
        WM_SETFOCUS => {
            if let Some(list) = app::with_app(|a| a.list) {
                SetFocus(list);
            }
            0
        }
        WM_DPICHANGED => {
            let dpi = ((wparam >> 16) & 0xFFFF) as u32;
            app::with_app(|a| a.dpi = dpi);
            app::layout(hwnd);
            0
        }
        WM_GETMINMAXINFO => {
            let mmi = &mut *(lparam as *mut MINMAXINFO);
            let dpi = app::with_app(|a| a.dpi).unwrap_or(96);
            mmi.ptMinTrackSize.x = scale(640, dpi);
            mmi.ptMinTrackSize.y = scale(480, dpi);
            0
        }
        WM_DROPFILES => {
            let hdrop = wparam as HANDLE;
            let count = DragQueryFileW(hdrop, 0xFFFF_FFFF, std::ptr::null_mut(), 0);
            let mut paths: Vec<PathBuf> = Vec::new();
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0) as usize;
                let mut buf = vec![0u16; len + 1];
                DragQueryFileW(hdrop, i, buf.as_mut_ptr(), (len + 1) as u32);
                paths.push(PathBuf::from(from_wide(buf.as_ptr())));
            }
            DragFinish(hdrop);
            app::open_files(hwnd, &paths);
            0
        }
        WM_ACTIVATE => {
            // Some builds drop the dark caption when activation changes.
            theme::enable_dark_mode(hwnd);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            app::on_destroy();
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn register_class(
    hinst: HINSTANCE,
    name: &str,
    proc_: WNDPROC,
    cursor: HCURSOR,
    brush: HBRUSH,
    icon: HICON,
) -> bool {
    let mut class_name = wide(name);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS,
        lpfnWndProc: proc_,
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinst,
        hIcon: icon,
        hCursor: cursor,
        hbrBackground: brush,
        lpszMenuName: std::ptr::null(),
        lpszClassName: class_name.as_mut_ptr(),
        hIconSm: icon,
    };
    RegisterClassExW(&wc) != 0
}

fn main() {
    unsafe {
        // Before any window exists, so menus and controls are created dark.
        theme::init_dark_mode();

        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        let cl = app::parse_args(&args);

        if cl.output.is_some() || cl.csv.is_some() {
            AttachConsole(0xFFFF_FFFF);
            std::process::exit(app::run_headless(&cl));
        }

        let hinst = GetModuleHandleW(std::ptr::null());
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_STANDARD_CLASSES | ICC_BAR_CLASSES,
        };
        InitCommonControlsEx(&icc);

        let arrow = LoadCursorW(std::ptr::null_mut(), IDC_ARROW);
        let mut icon = LoadIconW(hinst, 1 as usize as *const u16);
        if icon.is_null() {
            icon = LoadIconW(std::ptr::null_mut(), IDI_APPLICATION);
        }

        let bg_brush = theme::brush(theme::BG);
        let panel_brush = theme::brush(theme::PANEL);
        let toolbar_brush = theme::brush(theme::TOOLBAR);
        let header_brush = theme::brush(theme::HEADER);
        let canvas_brush = theme::brush(theme::CANVAS);

        let ok = register_class(hinst, MAIN_CLASS, Some(main_proc), arrow, bg_brush, icon)
            && register_class(
                hinst,
                preview::PREVIEW_CLASS,
                Some(preview::preview_proc),
                arrow,
                canvas_brush,
                icon,
            )
            && register_class(
                hinst,
                SPLITTER_CLASS,
                Some(app::splitter_proc),
                LoadCursorW(std::ptr::null_mut(), IDC_SIZEWE),
                bg_brush,
                icon,
            )
            && register_class(
                hinst,
                dialogs::SETTINGS_CLASS,
                Some(dialogs::settings_proc),
                arrow,
                panel_brush,
                icon,
            )
            && register_class(
                hinst,
                chrome::TOOLBAR_CLASS,
                Some(chrome::toolbar_proc),
                arrow,
                toolbar_brush,
                icon,
            )
            && register_class(
                hinst,
                chrome::PANELBAR_CLASS,
                Some(chrome::panelbar_proc),
                arrow,
                header_brush,
                icon,
            )
            && register_class(
                hinst,
                chrome::STATUS_CLASS,
                Some(chrome::status_proc),
                arrow,
                toolbar_brush,
                icon,
            );

        if !ok {
            msgbox(
                std::ptr::null_mut(),
                "Call to RegisterClassEx failed!",
                MB_ICONWARNING,
            );
            return;
        }

        let menu = app::build_menu();
        let mut caption = wide(PROGRAM_CAPTION);
        let mut class_name = wide(MAIN_CLASS);

        let hwnd = CreateWindowExW(
            0,
            class_name.as_mut_ptr(),
            caption.as_mut_ptr(),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1080,
            720,
            std::ptr::null_mut(),
            menu,
            hinst,
            std::ptr::null(),
        );

        if hwnd.is_null() {
            msgbox(
                std::ptr::null_mut(),
                "Call to CreateWindow failed!",
                MB_ICONWARNING,
            );
            return;
        }

        theme::enable_dark_mode(hwnd);
        app::create(hwnd, &cl);
        DragAcceptFiles(hwnd, 1);

        if !cl.paths.is_empty() {
            app::open_files(hwnd, &cl.paths);
        }

        ShowWindow(hwnd, SW_SHOWMAXIMIZED);
        // Re-apply once visible: some builds only honour the caption attribute then.
        theme::enable_dark_mode(hwnd);
        UpdateWindow(hwnd);

        let accels = [
            Accel {
                f_virt: FVIRTKEY | FCONTROL,
                key: 0x4F, // O
                cmd: app::MENU_OPEN as u16,
            },
            Accel {
                f_virt: FVIRTKEY | FCONTROL,
                key: 0xBB, // +
                cmd: app::MENU_ZOOM_IN as u16,
            },
            Accel {
                f_virt: FVIRTKEY | FCONTROL,
                key: 0xBD, // -
                cmd: app::MENU_ZOOM_OUT as u16,
            },
            Accel {
                f_virt: FVIRTKEY | FCONTROL,
                key: 0x30, // 0
                cmd: app::MENU_ZOOM_RESET as u16,
            },
        ];
        let accel =
            CreateAcceleratorTableW(accels.as_ptr() as *const ACCEL, accels.len() as i32);

        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
            if r <= 0 {
                break;
            }
            if TranslateAcceleratorW(hwnd, accel, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }

        if !accel.is_null() {
            DestroyAcceleratorTable(accel);
        }
    }
}
