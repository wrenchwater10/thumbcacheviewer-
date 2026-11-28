// Custom-drawn dark data grid.
//
// This replaces the common listview entirely: rows, header, selection, hover and
// both scrollbars are painted here, so no system colour (in particular the blue
// selection highlight) and no light scrollbar can ever appear.

#![allow(dead_code)]

use crate::app;
use crate::win::NUM_COLUMNS;
use crate::theme;
use crate::win::*;
use std::cell::RefCell;
use std::collections::BTreeSet;

pub const GRID_CLASS: &str = "tcv_grid";
pub const HEADER_H: i32 = 24;
pub const ROW_H: i32 = 22;
const SCROLLBAR: i32 = 12;

#[derive(Default)]
pub struct Grid {
    pub hwnd: HWND,
    pub widths: [i32; NUM_COLUMNS],
    pub count: usize,
    pub top: usize,
    pub selected: BTreeSet<usize>,
    pub scroll_x: i32,
    drag: Option<Drag>,
}

#[derive(Clone, Copy)]
enum Drag {
    Vertical(i32),
    Horizontal(i32),
}

thread_local! {
    static GRID: RefCell<Option<Grid>> = const { RefCell::new(None) };
}

pub fn with_grid<R>(f: impl FnOnce(&mut Grid) -> R) -> Option<R> {
    GRID.with(|cell| cell.borrow_mut().as_mut().map(f))
}

impl Grid {
    pub fn selected_count(&self) -> usize {
        self.selected.len()
    }

    pub fn first_selected(&self) -> Option<usize> {
        self.selected.iter().next().copied()
    }
}

pub fn selected_rows() -> Vec<usize> {
    with_grid(|grid| grid.selected.iter().copied().collect()).unwrap_or_default()
}

pub fn selected_count() -> usize {
    with_grid(|grid| grid.selected.len()).unwrap_or(0)
}

pub fn row_count() -> usize {
    with_grid(|grid| grid.count).unwrap_or(0)
}

pub unsafe fn init(hwnd: HWND, widths: [i32; NUM_COLUMNS]) {
    GRID.with(|cell| {
        *cell.borrow_mut() = Some(Grid {
            hwnd,
            widths,
            ..Default::default()
        });
    });
}

pub unsafe fn invalidate() {
    with_grid(|grid| {
        if !grid.hwnd.is_null() {
            InvalidateRect(grid.hwnd, std::ptr::null(), 0);
        }
    });
}

pub unsafe fn set_count(count: usize) {
    with_grid(|grid| {
        grid.count = count;
        grid.selected.clear();
        grid.top = grid.top.min(count.saturating_sub(1));
        if grid.top + 1 > count {
            grid.top = 0;
        }
        let hwnd = grid.hwnd;
        if !hwnd.is_null() {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    });
}

pub unsafe fn set_selection(rows: &[usize]) {
    with_grid(|grid| {
        grid.selected = rows.iter().copied().collect();
        if let Some(&first) = rows.first() {
            ensure_visible(grid, first);
        }
        let hwnd = grid.hwnd;
        if !hwnd.is_null() {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    });
}

pub unsafe fn select_all() {
    let count = row_count();
    with_grid(|grid| {
        grid.selected = (0..grid.count).collect();
        let hwnd = grid.hwnd;
        if !hwnd.is_null() {
            InvalidateRect(hwnd, std::ptr::null(), 0);
        }
    });
    let _ = count;
    app::on_selection_changed();
}

fn ensure_visible(grid: &mut Grid, row: usize) {
    let hwnd = grid.hwnd;
    if hwnd.is_null() {
        return;
    }
    unsafe {
        let dpi = GetDpiForWindow(hwnd);
        let row_h = scale(ROW_H, dpi);
        let mut rc: RECT = std::mem::zeroed();
        GetClientRect(hwnd, &mut rc);
        let visible = ((rc.bottom - scale(HEADER_H, dpi)) / row_h.max(1)).max(1) as usize;
        if row < grid.top {
            grid.top = row;
        } else if row >= grid.top + visible {
            grid.top = row + 1 - visible;
        }
    }
}

// ---------------------------------------------------------------- geometry

struct Metrics {
    client: RECT,
    header_h: i32,
    row_h: i32,
    bar: i32,
    view_w: i32,
    view_h: i32,
    total_w: i32,
    need_v: bool,
    need_h: bool,
    visible_rows: usize,
}

unsafe fn metrics(grid: &Grid, hwnd: HWND) -> Metrics {
    let dpi = GetDpiForWindow(hwnd);
    let mut client: RECT = std::mem::zeroed();
    GetClientRect(hwnd, &mut client);

    let header_h = scale(HEADER_H, dpi);
    let row_h = scale(ROW_H, dpi).max(1);
    let bar = scale(SCROLLBAR, dpi);
    let total_w: i32 = grid.widths.iter().sum();
    let total_h = grid.count as i32 * row_h;

    let need_v = total_h > client.bottom - header_h;
    let view_w = client.right - if need_v { bar } else { 0 };
    let need_h = total_w > view_w;
    let view_h = client.bottom - header_h - if need_h { bar } else { 0 };
    let visible_rows = ((view_h / row_h).max(0)) as usize;

    Metrics {
        client,
        header_h,
        row_h,
        bar,
        view_w,
        view_h,
        total_w,
        need_v,
        need_h,
        visible_rows,
    }
}

fn vbar_thumb(m: &Metrics, top: usize, count: usize) -> (i32, i32) {
    let track = m.view_h.max(1);
    let total = (count as i32 * m.row_h).max(track);
    let thumb = ((track as i64 * track as i64) / total as i64) as i32;
    let thumb = thumb.clamp(scale(24, 96), track);
    let max_top = count.saturating_sub(m.visible_rows.max(1)) as i64;
    let pos = if max_top == 0 {
        0
    } else {
        ((top as i64 * (track - thumb) as i64) / max_top) as i32
    };
    (thumb, pos)
}

fn hbar_thumb(m: &Metrics, scroll_x: i32) -> (i32, i32) {
    let track = m.view_w.max(1);
    let thumb = ((track as i64 * track as i64) / m.total_w.max(1) as i64) as i32;
    let thumb = thumb.clamp(scale(24, 96), track);
    let max_scroll = (m.total_w - m.view_w).max(1);
    let pos = ((scroll_x as i64 * (track - thumb) as i64) / max_scroll as i64) as i32;
    (thumb, pos.clamp(0, (track - thumb).max(0)))
}

// ------------------------------------------------------------------ painting

unsafe fn paint(hwnd: HWND) {
    let mut ps: PAINTSTRUCT = std::mem::zeroed();
    let hdc = BeginPaint(hwnd, &mut ps);

    let dpi = GetDpiForWindow(hwnd);
    let snapshot = with_grid(|grid| {
        (
            grid.count,
            grid.top,
            grid.selected.clone(),
            grid.scroll_x,
            grid.widths,
        )
    });
    let Some((count, top, selected, scroll_x, widths)) = snapshot else {
        EndPaint(hwnd, &ps);
        return;
    };

    let grid = Grid {
        count,
        top,
        selected: selected.clone(),
        scroll_x,
        widths,
        ..Default::default()
    };
    let m = metrics(&grid, hwnd);

    theme::fill(hdc, &m.client, theme::PANEL);

    // ---- header
    let header = RECT {
        left: 0,
        top: 0,
        right: m.client.right,
        bottom: m.header_h,
    };
    theme::fill(hdc, &header, theme::HEADER);
    SaveDC(hdc);
    IntersectClipRect(hdc, 0, 0, m.view_w, m.header_h);
    let mut x = -scroll_x;
    for (col, title) in app::COLUMNS.iter().enumerate() {
        let width = widths[col];
        let cell = RECT {
            left: x,
            top: 0,
            right: x + width,
            bottom: m.header_h,
        };
        theme::text_with_font(
            hdc,
            title,
            RECT {
                left: cell.left + scale(8, dpi),
                top: cell.top,
                right: cell.right - scale(6, dpi),
                bottom: cell.bottom,
            },
            theme::TEXT,
            dpi,
            9,
            DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        theme::vline(hdc, cell.right - 1, 0, m.header_h, theme::BORDER);
        x += width;
    }
    RestoreDC(hdc, -1);
    theme::hline(hdc, 0, m.client.right, m.header_h - 1, theme::BORDER);

    // ---- rows
    let rows: Vec<Vec<String>> = if m.visible_rows == 0 {
        Vec::new()
    } else {
        let last = (top + m.visible_rows + 1).min(count);
        app::with_app(|app| app::row_texts(app, top, last)).unwrap_or_default()
    };

    SaveDC(hdc);
    IntersectClipRect(hdc, 0, m.header_h, m.view_w, m.header_h + m.view_h.max(0));
    for (offset, texts) in rows.iter().enumerate() {
        let row = top + offset;
        if row >= count {
            break;
        }
        let y = m.header_h + offset as i32 * m.row_h;
        let band = RECT {
            left: 0,
            top: y,
            right: m.client.right,
            bottom: y + m.row_h,
        };
        if selected.contains(&row) {
            theme::fill(hdc, &band, theme::SELECT);
        } else if row % 2 == 1 {
            theme::fill(hdc, &band, theme::ROW_ALT);
        }

        let color = if selected.contains(&row) {
            theme::TEXT_BRIGHT
        } else {
            theme::TEXT
        };
        let mut cx = -scroll_x;
        for (col, text) in texts.iter().enumerate() {
            let width = widths[col];
            if cx + width > 0 && cx < m.view_w {
                theme::text_with_font(
                    hdc,
                    text,
                    RECT {
                        left: cx + scale(8, dpi),
                        top: y,
                        right: cx + width - scale(6, dpi),
                        bottom: y + m.row_h,
                    },
                    color,
                    dpi,
                    9,
                    DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | DT_NOPREFIX,
                );
            }
            cx += width;
        }
    }
    RestoreDC(hdc, -1);

    // ---- scrollbars
    if m.need_v {
        let track = RECT {
            left: m.client.right - m.bar,
            top: m.header_h,
            right: m.client.right,
            bottom: m.header_h + m.view_h,
        };
        theme::fill(hdc, &track, theme::PANEL);
        theme::vline(hdc, track.left, track.top, track.bottom, theme::BORDER);
        let (thumb, pos) = vbar_thumb(&m, top, count);
        let thumb_rect = RECT {
            left: track.left + scale(2, dpi),
            top: track.top + pos,
            right: track.right - scale(2, dpi),
            bottom: track.top + pos + thumb,
        };
        theme::fill(hdc, &thumb_rect, theme::SCROLL_THUMB);
    }
    if m.need_h {
        let track = RECT {
            left: 0,
            top: m.client.bottom - m.bar,
            right: m.view_w,
            bottom: m.client.bottom,
        };
        theme::fill(hdc, &track, theme::PANEL);
        theme::hline(hdc, track.left, track.right, track.top, theme::BORDER);
        let (thumb, pos) = hbar_thumb(&m, scroll_x);
        let thumb_rect = RECT {
            left: track.left + pos,
            top: track.top + scale(2, dpi),
            right: track.left + pos + thumb,
            bottom: track.bottom - scale(2, dpi),
        };
        theme::fill(hdc, &thumb_rect, theme::SCROLL_THUMB);
    }

    EndPaint(hwnd, &ps);
}

// -------------------------------------------------------------- interaction

unsafe fn select_row(hwnd: HWND, row: usize, additive: bool) {
    with_grid(|grid| {
        if additive {
            if !grid.selected.insert(row) {
                grid.selected.remove(&row);
            }
        } else {
            grid.selected.clear();
            grid.selected.insert(row);
        }
        ensure_visible(grid, row);
        InvalidateRect(hwnd, std::ptr::null(), 0);
    });
    app::on_selection_changed();
}

unsafe fn scroll_by(hwnd: HWND, delta: i32) {
    with_grid(|grid| {
        let m = metrics(grid, hwnd);
        let max_top = grid.count.saturating_sub(m.visible_rows.max(1)) as i32;
        grid.top = (grid.top as i32 + delta).clamp(0, max_top.max(0)) as usize;
        InvalidateRect(hwnd, std::ptr::null(), 0);
    });
}

pub unsafe extern "system" fn grid_proc(
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
        WM_SIZE => {
            with_grid(|grid| {
                let m = metrics(grid, hwnd);
                let max_top = grid.count.saturating_sub(m.visible_rows.max(1));
                if grid.top > max_top {
                    grid.top = max_top;
                }
                let max_scroll = (m.total_w - m.view_w).max(0);
                if grid.scroll_x > max_scroll {
                    grid.scroll_x = max_scroll;
                }
            });
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_MOUSEWHEEL => {
            let delta = -(((wparam >> 16) as i16) as i32) / 40;
            scroll_by(hwnd, delta.clamp(-5, 5) * 3);
            0
        }
        WM_VSCROLL => {
            scroll_by(hwnd, 3);
            0
        }
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;

            let snapshot = with_grid(|grid| (grid.count, grid.top, grid.scroll_x, grid.widths));
            let Some((count, top, scroll_x, widths)) = snapshot else {
                return 0;
            };
            let mut grid = Grid {
                hwnd,
                count,
                top,
                scroll_x,
                widths,
                ..Default::default()
            };
            let m = metrics(&grid, hwnd);
            let _ = &mut grid;

            // Scrollbars first.
            if m.need_v {
                let track = RECT {
                    left: m.client.right - m.bar,
                    top: m.header_h,
                    right: m.client.right,
                    bottom: m.header_h + m.view_h,
                };
                if x >= track.left {
                    let (thumb, pos) = vbar_thumb(&m, top, count);
                    if y >= track.top + pos && y < track.top + pos + thumb {
                        with_grid(|g| g.drag = Some(Drag::Vertical(y - (track.top + pos))));
                    } else if y >= track.top {
                        let page = m.visible_rows.max(1) as i32;
                        scroll_by(hwnd, if y < track.top + pos { -page } else { page });
                    }
                    return 0;
                }
            }
            if m.need_h && y >= m.client.bottom - m.bar {
                let (thumb, pos) = hbar_thumb(&m, scroll_x);
                if x >= pos && x < pos + thumb {
                    with_grid(|g| g.drag = Some(Drag::Horizontal(x - pos)));
                }
                return 0;
            }

            // Header: clicking a column cycles its number format.
            if y < m.header_h {
                let mut cx = -scroll_x;
                for col in 0..NUM_COLUMNS {
                    if x >= cx && x < cx + widths[col] {
                        app::cycle_column_format(col);
                        break;
                    }
                    cx += widths[col];
                }
                return 0;
            }

            let row = top + ((y - m.header_h) / m.row_h) as usize;
            if row < count {
                let additive = (GetKeyState(VK_CONTROL as i32) as u16 & 0x8000) != 0;
                select_row(hwnd, row, additive);
            }
            0
        }
        WM_MOUSEMOVE => {
            let x = (lparam as i32) as i16 as i32;
            let y = ((lparam as i32) >> 16) as i16 as i32;
            let drag = with_grid(|grid| grid.drag).flatten();
            match drag {
                Some(Drag::Vertical(offset)) => {
                    let snapshot = with_grid(|grid| (grid.count, grid.top, grid.widths));
                    if let Some((count, _, widths)) = snapshot {
                        let grid = Grid {
                            hwnd,
                            count,
                            widths,
                            ..Default::default()
                        };
                        let m = metrics(&grid, hwnd);
                        let (thumb, _) = vbar_thumb(&m, 0, count);
                        let span = (m.view_h - thumb).max(1);
                        let max_top = count.saturating_sub(m.visible_rows.max(1)) as i32;
                        let pos = (y - m.header_h - offset).clamp(0, span);
                        let new_top = ((pos as i64 * max_top as i64) / span as i64) as i32;
                        with_grid(|g| g.top = new_top.max(0) as usize);
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
                Some(Drag::Horizontal(offset)) => {
                    let snapshot = with_grid(|grid| grid.widths);
                    if let Some(widths) = snapshot {
                        let grid = Grid {
                            hwnd,
                            widths,
                            ..Default::default()
                        };
                        let m = metrics(&grid, hwnd);
                        let (thumb, _) = hbar_thumb(&m, 0);
                        let span = (m.view_w - thumb).max(1);
                        let max_scroll = (m.total_w - m.view_w).max(0);
                        let pos = (x - offset).clamp(0, span);
                        let new_scroll = ((pos as i64 * max_scroll as i64) / span as i64) as i32;
                        with_grid(|g| g.scroll_x = new_scroll);
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
                None => {}
            }
            0
        }
        WM_LBUTTONUP => {
            with_grid(|grid| grid.drag = None);
            0
        }
        WM_KEYDOWN => {
            let key = wparam as u16;
            let first = with_grid(|grid| grid.selected.iter().next().copied())
                .flatten()
                .unwrap_or(0);
            let count = row_count();
            let target = match key {
                VK_UP => Some(first.saturating_sub(1)),
                VK_DOWN => Some((first + 1).min(count.saturating_sub(1))),
                VK_HOME => Some(0),
                VK_END => Some(count.saturating_sub(1)),
                VK_PRIOR => Some(first.saturating_sub(10)),
                VK_NEXT => Some((first + 10).min(count.saturating_sub(1))),
                _ => None,
            };
            if let Some(row) = target {
                if count > 0 {
                    select_row(hwnd, row, false);
                }
            }
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
