//! A single 32x32 premultiplied-alpha marker; never a fullscreen paint surface.
use std::ptr::{null, null_mut};
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*,
};
pub struct Marker {
    pub hwnd: HWND,
    dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    bits: *mut u32,
    preview: bool,
    width: i32,
    height: i32,
}
impl Marker {
    pub unsafe fn new(owner: HWND, preview: bool) -> Option<Self> {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        let (width, height) = if preview { (42, 30) } else { (32, 32) };
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TRANSPARENT | WS_EX_TOPMOST,
            crate::wide("Yami106Calibration").as_ptr(),
            crate::wide("Yami calibration").as_ptr(),
            WS_POPUP,
            p.x - 16,
            p.y - 16,
            width,
            height,
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            null(),
        );
        if hwnd.is_null() {
            return None;
        }
        let dc = CreateCompatibleDC(null_mut());
        let mut bits = null_mut();
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..std::mem::zeroed()
            },
            ..std::mem::zeroed()
        };
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
        if dc.is_null() || bitmap.is_null() || bits.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            if !dc.is_null() {
                DeleteDC(dc);
            }
            DestroyWindow(hwnd);
            return None;
        }
        let old = SelectObject(dc, bitmap);
        let pixels = std::slice::from_raw_parts_mut(bits as *mut u32, (width * height) as usize);
        pixels.fill(0);
        if !preview {
            for y in 0..32 {
                for x in 0..32 {
                    let dx = x as f32 - 15.5;
                    let dy = y as f32 - 15.5;
                    // Antialiased premultiplied-alpha red X with a restrained halo.
                    let distance = (dx.abs() - dy.abs()).abs() * std::f32::consts::FRAC_1_SQRT_2;
                    let extent = dx.abs().max(dy.abs());
                    let end = ((13.5 - extent) / 2.).clamp(0., 1.);
                    let core = (1.5 - distance).clamp(0., 1.);
                    let glow = 0.30 * (-distance * distance / 7.).exp();
                    let alpha = ((core.max(glow) * end) * 255.).round() as u32;
                    if alpha != 0 {
                        pixels[y * 32 + x] = (alpha << 24)
                            | (alpha << 16)
                            | ((57 * alpha / 255) << 8)
                            | (76 * alpha / 255);
                    }
                }
            }
        }
        let marker = Self {
            hwnd,
            dc,
            bitmap,
            old,
            bits: bits as *mut u32,
            preview,
            width,
            height,
        };
        if !marker.update() {
            drop(marker);
            return None;
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        Some(marker)
    }
    pub unsafe fn color(&mut self, rgb: u32) {
        if !self.preview {
            return;
        }
        let pixels = std::slice::from_raw_parts_mut(self.bits, (self.width * self.height) as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                pixels[(y * self.width + x) as usize] =
                    if x == 0 || y == 0 || x == self.width - 1 || y == self.height - 1 {
                        0xff92929b
                    } else {
                        0xff000000 | (rgb & 0xffffff)
                    };
            }
        }
    }
    pub unsafe fn update(&self) -> bool {
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        if self.preview {
            let work = crate::windowing::work_area(MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST));
            p.x = if p.x + 20 + self.width <= work.right {
                p.x + 20
            } else {
                p.x - 20 - self.width
            };
            p.y = if p.y + 20 + self.height <= work.bottom {
                p.y + 20
            } else {
                p.y - 20 - self.height
            };
        } else {
            p.x -= 16;
            p.y -= 16;
        }
        let size = SIZE {
            cx: self.width,
            cy: self.height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        UpdateLayeredWindow(
            self.hwnd,
            null_mut(),
            &p,
            &size,
            self.dc,
            &source,
            0,
            &blend,
            ULW_ALPHA,
        ) != 0
    }
}
impl Drop for Marker {
    fn drop(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
            DestroyWindow(self.hwnd);
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
