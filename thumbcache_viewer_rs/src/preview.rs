// The embedded image preview pane: decoding, zooming, panning, painting,
// clipboard copy and image export.
//
// Everything is drawn into an off-screen bitmap first and then blitted in one
// piece, so panning and resizing never flicker or leave partial frames behind.

#![allow(dead_code)]

use crate::app::{with_app, MENU_COPY_IMAGE, MENU_EXPORT_IMAGE};
use crate::theme;
use crate::win::*;
use std::path::Path;

pub const PREVIEW_CLASS: &str = "tcv_preview";

const MIN_SCALE: f32 = 0.1;
const MAX_SCALE: f32 = 25.0;

struct Dib {
    hbmp: HBITMAP,
    bits: *mut u8,
    w: i32,
    h: i32,
}

impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.hbmp as HGDIOBJ);
        }
    }
}

unsafe fn create_ui_font(dpi: u32, points: i32) -> HFONT {
    let height = -(((points as i64) * (dpi as i64) + 36) / 72) as i32;
    let face = wide("Segoe UI");
    CreateFontW(
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
    )
}

pub struct Preview {
    pub hwnd: HWND,
    dib: Option<Dib>,
    font: HFONT,
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub fit_mode: bool,
    pub zoom_step: f32,
    pub origin: (f32, f32),
    pub dragging: bool,
    drag_start: (i32, i32),
    drag_origin: (f32, f32),
    pub message: String,
}

impl Preview {
    pub fn new(hwnd: HWND) -> Self {
        let dpi = unsafe { GetDpiForWindow(hwnd) };
        Preview {
            hwnd,
            dib: None,
            font: unsafe { create_ui_font(dpi, 9) },
            pixels: Vec::new(),
            width: 0,
            height: 0,
            scale: 1.0,
            fit_mode: true,
            zoom_step: 0.5,
            origin: (0.0, 0.0),
            dragging: false,
            drag_start: (0, 0),
            drag_origin: (0.0, 0.0),
            message: "Select a cache entry to view its image.".to_string(),
        }
    }

    pub fn has_image(&self) -> bool {
        self.dib.is_some()
    }

    pub fn set_message(&mut self, text: &str) {
        self.dib = None;
        self.pixels.clear();
        self.width = 0;
        self.height = 0;
        self.message = text.to_string();
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    pub fn set_image(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        self.width = width;
        self.height = height;
        self.pixels = rgba;
        self.message.clear();
        self.rebuild_dib();
        self.fit_mode = true;
        self.fit();
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    fn rebuild_dib(&mut self) {
        self.dib = None;
        if self.width == 0 || self.height == 0 {
            return;
        }
        unsafe {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width as i32;
            bmi.bmiHeader.biHeight = -(self.height as i32);
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;
            bmi.bmiHeader.biSizeImage = self.width * self.height * 4;

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let hbmp = CreateDIBSection(
                std::ptr::null_mut(),
                &bmi,
                DIB_RGB_COLORS,
                &mut bits,
                std::ptr::null_mut(),
                0,
            );
            if hbmp.is_null() || bits.is_null() {
                self.message = "Unable to allocate image buffer.".to_string();
                return;
            }

            let dst = bits as *mut u8;
            let count = (self.width * self.height) as usize;
            for i in 0..count {
                let s = i * 4;
                *dst.add(s) = self.pixels[s + 2];
                *dst.add(s + 1) = self.pixels[s + 1];
                *dst.add(s + 2) = self.pixels[s];
                *dst.add(s + 3) = self.pixels[s + 3];
            }

            self.dib = Some(Dib {
                hbmp,
                bits: dst,
                w: self.width as i32,
                h: self.height as i32,
            });
        }
    }

    fn client_size(&self) -> (i32, i32) {
        unsafe {
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut rc);
            (rc.right - rc.left, rc.bottom - rc.top)
        }
    }

    pub fn center(&mut self) {
        let (cw, ch) = self.client_size();
        let w = self.width as f32 * self.scale;
        let h = self.height as f32 * self.scale;
        self.origin = (((cw as f32) - w) / 2.0, ((ch as f32) - h) / 2.0);
    }

    /// Keeps the image where the user left it and only re-centres it when it
    /// fits; avoids the jumpy, half-drawn frames resizing used to produce.
    pub fn on_resize(&mut self) {
        if self.fit_mode {
            self.fit();
            unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
            return;
        }
        let (cw, ch) = self.client_size();
        if self.has_image() {
            let iw = self.width as f32 * self.scale;
            let ih = self.height as f32 * self.scale;
            if iw <= cw as f32 && ih <= ch as f32 {
                self.center();
            } else {
                let min_x = (cw as f32 - iw).min(0.0);
                let min_y = (ch as f32 - ih).min(0.0);
                self.origin.0 = self.origin.0.clamp(min_x, 0.0);
                self.origin.1 = self.origin.1.clamp(min_y, 0.0);
            }
        }
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    /// Scales the image down so the whole picture fits the pane, never enlarging
    /// it beyond 100%, and keeps it fitted when the pane is resized.
    pub fn fit(&mut self) {
        let (cw, ch) = self.client_size();
        if self.width == 0 || self.height == 0 || cw <= 8 || ch <= 8 {
            self.center();
            return;
        }
        let sx = cw as f32 / self.width as f32;
        let sy = ch as f32 / self.height as f32;
        self.scale = sx.min(sy).clamp(MIN_SCALE, 1.0);
        self.center();
    }

    pub fn reset_zoom(&mut self) {
        self.fit_mode = true;
        self.fit();
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    pub fn zoom(&mut self, direction: f32) {
        let (cw, ch) = self.client_size();
        self.zoom_at(direction, (cw as f32 / 2.0, ch as f32 / 2.0));
        unsafe { InvalidateRect(self.hwnd, std::ptr::null(), 0) };
    }

    fn zoom_at(&mut self, direction: f32, anchor: (f32, f32)) {
        if !self.has_image() {
            return;
        }
        let old = self.scale;
        let new = (old + self.zoom_step * direction).clamp(MIN_SCALE, MAX_SCALE);
        if (new - old).abs() < 1e-6 {
            return;
        }
        self.fit_mode = false;
        let k = new / old;
        self.origin.0 = anchor.0 - (anchor.0 - self.origin.0) * k;
        self.origin.1 = anchor.1 - (anchor.1 - self.origin.1) * k;
        self.scale = new;
    }

    // ------------------------------------------------------------- painting

    pub fn paint(&self) {
        unsafe {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(self.hwnd, &mut ps);

            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(self.hwnd, &mut rc);
            let cw = rc.right;
            let ch = rc.bottom;

            if cw > 0 && ch > 0 {
                let mem = CreateCompatibleDC(hdc);
                let back = CreateCompatibleBitmap(hdc, cw, ch);

                if !mem.is_null() && !back.is_null() {
                    let old_bitmap = SelectObject(mem, back as HGDIOBJ);
                    let old_font = if self.font.is_null() {
                        std::ptr::null_mut()
                    } else {
                        SelectObject(mem, self.font as HGDIOBJ)
                    };

                    theme::fill(mem, &rc, theme::CANVAS);

                    match &self.dib {
                        Some(dib) => self.draw_image(mem, dib, cw, ch),
                        None => self.draw_message(mem, cw, ch),
                    }

                    // One blit for the whole frame: no flicker, no leftovers.
                    BitBlt(hdc, 0, 0, cw, ch, mem, 0, 0, SRCCOPY);

                    if !old_font.is_null() {
                        SelectObject(mem, old_font);
                    }
                    SelectObject(mem, old_bitmap);
                    DeleteObject(back as HGDIOBJ);
                    DeleteDC(mem);
                } else {
                    if !back.is_null() {
                        DeleteObject(back as HGDIOBJ);
                    }
                    if !mem.is_null() {
                        DeleteDC(mem);
                    }
                }
            }

            EndPaint(self.hwnd, &ps);
        }
    }

    unsafe fn draw_image(&self, hdc: HDC, dib: &Dib, cw: i32, ch: i32) {
        let dw = ((dib.w as f32 * self.scale).round() as i32).max(1);
        let dh = ((dib.h as f32 * self.scale).round() as i32).max(1);
        let dx = self.origin.0.round() as i32;
        let dy = self.origin.1.round() as i32;

        let mut bmi: BITMAPINFO = std::mem::zeroed();
        bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bmi.bmiHeader.biWidth = dib.w;
        bmi.bmiHeader.biHeight = -dib.h;
        bmi.bmiHeader.biPlanes = 1;
        bmi.bmiHeader.biBitCount = 32;
        bmi.bmiHeader.biCompression = BI_RGB;
        bmi.bmiHeader.biSizeImage = (dib.w * dib.h * 4) as u32;

        SetStretchBltMode(hdc, HALFTONE);
        SetBrushOrgEx(hdc, 0, 0, std::ptr::null_mut());
        StretchDIBits(
            hdc,
            dx,
            dy,
            dw,
            dh,
            0,
            0,
            dib.w,
            dib.h,
            dib.bits as *const core::ffi::c_void,
            &bmi,
            DIB_RGB_COLORS,
            SRCCOPY,
        );

        // A one pixel frame makes the document read as floating on the canvas.
        let border = RECT {
            left: dx - 1,
            top: dy - 1,
            right: dx + dw + 1,
            bottom: dy + dh + 1,
        };
        theme::frame(hdc, &border, theme::IMAGE_FRAME);

        // Zoom readout in the bottom-left corner, in the UI font.
        let label = format!("{}%", (self.scale * 100.0).round() as i32);
        let mut wtext = wide(&label);
        let mut text_rc = RECT {
            left: 12,
            top: ch - 32,
            right: cw - 12,
            bottom: ch - 8,
        };
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, theme::TEXT_DIM);
        DrawTextW(
            hdc,
            wtext.as_mut_ptr(),
            -1,
            &mut text_rc,
            DT_LEFT | DT_SINGLELINE | DT_NOPREFIX,
        );
    }

    unsafe fn draw_message(&self, hdc: HDC, cw: i32, ch: i32) {
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, theme::TEXT_DIM);

        let mut text = wide(&self.message);
        let width = (cw - 32).max(32);

        // Measure first so the text can be vertically centred.
        let mut measure = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: 0,
        };
        DrawTextW(
            hdc,
            text.as_mut_ptr(),
            -1,
            &mut measure,
            DT_CENTER | DT_WORDBREAK | DT_CALCRECT | DT_NOPREFIX,
        );
        let text_height = (measure.bottom - measure.top).max(1);

        let mut rc = RECT {
            left: 16,
            top: ((ch - text_height) / 2).max(0),
            right: 16 + width,
            bottom: ch,
        };
        DrawTextW(
            hdc,
            text.as_mut_ptr(),
            -1,
            &mut rc,
            DT_CENTER | DT_WORDBREAK | DT_NOPREFIX,
        );
    }

    // ------------------------------------------------------- clipboard/export

    /// Builds a packed bottom-up 32bpp DIB (BITMAPINFOHEADER + pixels).
    fn dib_bytes(&self) -> Option<Vec<u8>> {
        if self.width == 0 || self.height == 0 {
            return None;
        }
        let w = self.width as usize;
        let h = self.height as usize;
        let header = 40usize;
        let mut out = vec![0u8; header + w * h * 4];

        let put = |out: &mut Vec<u8>, off: usize, v: u32| {
            out[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        put(&mut out, 0, 40);
        put(&mut out, 4, w as u32);
        put(&mut out, 8, h as u32);
        out[12..14].copy_from_slice(&1u16.to_le_bytes());
        out[14..16].copy_from_slice(&32u16.to_le_bytes());
        put(&mut out, 16, 0); // BI_RGB
        put(&mut out, 20, (w * h * 4) as u32);

        for y in 0..h {
            let dst_row = h - 1 - y;
            for x in 0..w {
                let s = (y * w + x) * 4;
                let d = header + (dst_row * w + x) * 4;
                out[d] = self.pixels[s + 2];
                out[d + 1] = self.pixels[s + 1];
                out[d + 2] = self.pixels[s];
                out[d + 3] = 0xFF;
            }
        }
        Some(out)
    }

    /// Returns a message describing why nothing was copied, or None on success.
    /// The caller shows it outside of the app borrow, so the modal dialog can
    /// never run while the app state is locked.
    pub unsafe fn copy_to_clipboard(&self, owner: HWND) -> Option<&'static str> {
        if !self.has_image() {
            return Some("There is no image to copy.");
        }

        let Some(dib) = self.dib_bytes() else {
            return Some("There is no image to copy.");
        };
        let png = encode_png(&self.pixels, self.width, self.height);

        if OpenClipboard(owner) == 0 {
            return Some("Unable to open the clipboard.");
        }
        EmptyClipboard();

        let hmem = GlobalAlloc(GMEM_MOVEABLE, dib.len());
        if !hmem.is_null() {
            let ptr = GlobalLock(hmem) as *mut u8;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(dib.as_ptr(), ptr, dib.len());
                GlobalUnlock(hmem);
                if SetClipboardData(CLIPBOARD_CF_DIB, hmem).is_null() {
                    GlobalFree(hmem);
                }
            } else {
                GlobalFree(hmem);
            }
        }

        if let Some(png) = png {
            let fmt_name = wide("PNG");
            let fmt = RegisterClipboardFormatW(fmt_name.as_ptr());
            if fmt != 0 {
                let hmem = GlobalAlloc(GMEM_MOVEABLE, png.len());
                if !hmem.is_null() {
                    let ptr = GlobalLock(hmem) as *mut u8;
                    if !ptr.is_null() {
                        std::ptr::copy_nonoverlapping(png.as_ptr(), ptr, png.len());
                        GlobalUnlock(hmem);
                        if SetClipboardData(fmt, hmem).is_null() {
                            GlobalFree(hmem);
                        }
                    } else {
                        GlobalFree(hmem);
                    }
                }
            }
        }

        CloseClipboard();
        None
    }

    pub fn export_to(&self, path: &Path) -> Option<Result<(), String>> {
        if !self.has_image() {
            return None;
        }
        let Some(img) = image::RgbaImage::from_raw(self.width, self.height, self.pixels.clone())
        else {
            return Some(Err("Unable to build the image.".to_string()));
        };
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let result = match ext.as_str() {
            "jpg" | "jpeg" => image::DynamicImage::ImageRgba8(img)
                .to_rgb8()
                .save(path)
                .map_err(|e| e.to_string()),
            _ => img.save(path).map_err(|e| e.to_string()),
        };
        Some(result)
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        unsafe {
            if !self.font.is_null() {
                DeleteObject(self.font as HGDIOBJ);
            }
        }
    }
}

fn encode_png(pixels: &[u8], w: u32, h: u32) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut out);
        encoder
            .write_image(pixels, w, h, image::ExtendedColorType::Rgba8)
            .ok()?;
    }
    Some(out)
}

// --------------------------------------------------------------------- messages

pub unsafe extern "system" fn preview_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            if with_app(|app| app.preview.hwnd == hwnd).unwrap_or(false) {
                with_app(|app| app.preview.paint());
            } else {
                // Could not borrow the app (a paint re-entered during a layout):
                // still fill the pane so no stale pixels are left behind.
                let mut ps: PAINTSTRUCT = std::mem::zeroed();
                let hdc = BeginPaint(hwnd, &mut ps);
                let mut rc: RECT = std::mem::zeroed();
                GetClientRect(hwnd, &mut rc);
                theme::fill(hdc, &rc, theme::CANVAS);
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_MOUSEWHEEL => {
            let delta = ((wparam >> 16) as i16) as f32 / 120.0;
            let mut pt = POINT {
                x: (lparam as i32) as i16 as i32,
                y: ((lparam as i32) >> 16) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut pt);
            with_app(|app| {
                app.preview.zoom_at(delta, (pt.x as f32, pt.y as f32));
                app.refresh_status();
                InvalidateRect(app.preview.hwnd, std::ptr::null(), 0);
            });
            0
        }
        WM_LBUTTONDOWN => {
            SetFocus(hwnd);
            with_app(|app| {
                app.preview.dragging = true;
                app.preview.fit_mode = false;
                app.preview.drag_start = (
                    (lparam as i32) as i16 as i32,
                    ((lparam as i32) >> 16) as i16 as i32,
                );
                app.preview.drag_origin = app.preview.origin;
            });
            SetCapture(hwnd);
            0
        }
        WM_MOUSEMOVE => {
            if GetCapture() == hwnd {
                with_app(|app| {
                    if app.preview.dragging {
                        let x = (lparam as i32) as i16 as i32;
                        let y = ((lparam as i32) >> 16) as i16 as i32;
                        let dx = (x - app.preview.drag_start.0) as f32;
                        let dy = (y - app.preview.drag_start.1) as f32;
                        app.preview.origin = (
                            app.preview.drag_origin.0 + dx,
                            app.preview.drag_origin.1 + dy,
                        );
                        InvalidateRect(app.preview.hwnd, std::ptr::null(), 0);
                    }
                });
            }
            0
        }
        WM_LBUTTONUP => {
            ReleaseCapture();
            with_app(|app| app.preview.dragging = false);
            0
        }
        WM_LBUTTONDBLCLK => {
            with_app(|app| {
                app.preview.reset_zoom();
                app.refresh_status();
            });
            0
        }
        WM_RBUTTONUP => {
            SetFocus(hwnd);
            let mut pt = POINT {
                x: (lparam as i32) as i16 as i32,
                y: ((lparam as i32) >> 16) as i16 as i32,
            };
            ClientToScreen(hwnd, &mut pt);
            let menu = CreatePopupMenu();
            AppendMenuW(menu, MF_STRING, MENU_COPY_IMAGE, wide("&Copy Image").as_ptr());
            AppendMenuW(menu, MF_STRING, MENU_EXPORT_IMAGE, wide("&Export Image...").as_ptr());
            let parent = GetParent(hwnd);
            SetForegroundWindow(parent);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                pt.x,
                pt.y,
                0,
                parent,
                std::ptr::null(),
            );
            DestroyMenu(menu);
            if cmd != 0 {
                PostMessageW(parent, WM_COMMAND, cmd as usize, 0);
            }
            0
        }
        WM_KEYDOWN => {
            with_app(|app| {
                match wparam as u32 {
                    VK_OEM_PLUS | VK_ADD => app.preview.zoom(1.0),
                    VK_OEM_MINUS | VK_SUBTRACT => app.preview.zoom(-1.0),
                    VK_0 | VK_NUMPAD0 => app.preview.reset_zoom(),
                    _ => {}
                }
                app.refresh_status();
            });
            0
        }
        WM_SIZE => {
            with_app(|app| app.preview.on_resize());
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
