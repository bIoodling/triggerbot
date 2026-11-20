use std::ffi::c_void;
type P = *mut c_void;
#[repr(C)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
#[repr(C)]
pub struct Startup {
    version: u32,
    callback: usize,
    thread: i32,
    codecs: i32,
}
#[link(name = "gdiplus")]
extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const Startup, output: P) -> i32;
    fn GdiplusShutdown(token: usize);
    fn GdipCreateFromHDC(dc: P, g: *mut P) -> i32;
    fn GdipDeleteGraphics(g: P) -> i32;
    fn GdipSetSmoothingMode(g: P, m: i32) -> i32;
    fn GdipSetTextRenderingHint(g: P, m: i32) -> i32;
    fn GdipScaleWorldTransform(g: P, x: f32, y: f32, o: i32) -> i32;
    fn GdipTranslateWorldTransform(g: P, x: f32, y: f32, order: i32) -> i32;
    fn GdipGraphicsClear(g: P, c: u32) -> i32;
    fn GdipCreatePath(mode: i32, p: *mut P) -> i32;
    fn GdipAddPathArc(p: P, x: f32, y: f32, w: f32, h: f32, start: f32, sweep: f32) -> i32;
    fn GdipClosePathFigure(p: P) -> i32;
    fn GdipDeletePath(p: P) -> i32;
    fn GdipFillPath(g: P, b: P, p: P) -> i32;
    fn GdipDrawPath(g: P, pen: P, p: P) -> i32;
    fn GdipCreateSolidFill(c: u32, b: *mut P) -> i32;
    fn GdipDeleteBrush(b: P) -> i32;
    fn GdipCreatePen1(c: u32, w: f32, u: i32, p: *mut P) -> i32;
    fn GdipDeletePen(p: P) -> i32;
    fn GdipDrawLine(g: P, p: P, x: f32, y: f32, x2: f32, y2: f32) -> i32;
    fn GdipDrawArc(g: P, p: P, x: f32, y: f32, w: f32, h: f32, a: f32, b: f32) -> i32;
    fn GdipFillEllipse(g: P, b: P, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipDrawEllipse(g: P, p: P, x: f32, y: f32, w: f32, h: f32) -> i32;
    fn GdipCreateFontFamilyFromName(n: *const u16, c: P, f: *mut P) -> i32;
    fn GdipDeleteFontFamily(f: P) -> i32;
    fn GdipCreateFont(f: P, s: f32, style: i32, u: i32, out: *mut P) -> i32;
    fn GdipDeleteFont(f: P) -> i32;
    fn GdipCreateStringFormat(flags: i32, lang: u16, out: *mut P) -> i32;
    fn GdipSetStringFormatAlign(f: P, a: i32) -> i32;
    fn GdipSetStringFormatLineAlign(f: P, a: i32) -> i32;
    fn GdipDrawString(g: P, t: *const u16, len: i32, f: P, r: *const Rect, format: P, b: P) -> i32;
    fn GdipDeleteStringFormat(p: P) -> i32;
    fn GdipCreateLineBrushFromRect(
        r: *const Rect,
        c1: u32,
        c2: u32,
        mode: i32,
        wrap: i32,
        out: *mut P,
    ) -> i32;
    fn GdipFillRectangle(g: P, b: P, x: f32, y: f32, w: f32, h: f32) -> i32;
}
pub const BG: u32 = 0xff09090d;
pub const PANEL: u32 = 0xff191923;
pub const SECOND: u32 = 0xff12121a;
pub const BORDER: u32 = 0xff30303d;
pub const TEXT: u32 = 0xfff5f3ff;
pub const DIM: u32 = 0xff9893aa;
pub const ACCENT: u32 = 0xff8b5cf6;
pub const HOVER: u32 = 0xffa78bfa;
pub const RED: u32 = 0xffe04457;
pub unsafe fn startup() -> usize {
    let mut t = 0;
    GdiplusStartup(
        &mut t,
        &Startup {
            version: 1,
            callback: 0,
            thread: 0,
            codecs: 0,
        },
        std::ptr::null_mut(),
    );
    t
}
pub unsafe fn shutdown(t: usize) {
    GdiplusShutdown(t)
}
pub struct Canvas {
    g: P,
}
impl Canvas {
    pub unsafe fn new(dc: P, s: f32) -> Self {
        let mut g = std::ptr::null_mut();
        GdipCreateFromHDC(dc, &mut g);
        GdipSetSmoothingMode(g, 4);
        GdipSetTextRenderingHint(g, 5);
        GdipScaleWorldTransform(g, s, s, 1);
        GdipGraphicsClear(g, BG);
        Self { g }
    }
    pub fn round(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32, edge: u32) {
        unsafe {
            let mut p = std::ptr::null_mut();
            GdipCreatePath(0, &mut p);
            for (xx, yy, a) in [
                (x, y, 180.),
                (x + w - r * 2., y, 270.),
                (x + w - r * 2., y + h - r * 2., 0.),
                (x, y + h - r * 2., 90.),
            ] {
                GdipAddPathArc(p, xx, yy, r * 2., r * 2., a, 90.);
            }
            GdipClosePathFigure(p);
            let mut b = std::ptr::null_mut();
            GdipCreateSolidFill(color, &mut b);
            GdipFillPath(self.g, b, p);
            GdipDeleteBrush(b);
            if edge != 0 {
                let mut pen = std::ptr::null_mut();
                GdipCreatePen1(edge, 1., 2, &mut pen);
                GdipDrawPath(self.g, pen, p);
                GdipDeletePen(pen);
            }
            GdipDeletePath(p);
        }
    }
    pub fn text(
        &self,
        t: &str,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        size: f32,
        color: u32,
        weight: u16,
        center: bool,
    ) {
        unsafe {
            let mut family = std::ptr::null_mut();
            GdipCreateFontFamilyFromName(
                crate::wide(if weight >= 600 {
                    "Segoe UI Semibold"
                } else {
                    "Segoe UI"
                })
                .as_ptr(),
                std::ptr::null_mut(),
                &mut family,
            );
            let mut font = std::ptr::null_mut();
            GdipCreateFont(family, size, 0, 2, &mut font);
            let mut format = std::ptr::null_mut();
            GdipCreateStringFormat(0x1000, 0, &mut format);
            GdipSetStringFormatLineAlign(format, 1);
            GdipSetStringFormatAlign(format, if center { 1 } else { 0 });
            let mut b = std::ptr::null_mut();
            GdipCreateSolidFill(color, &mut b);
            let txt: Vec<u16> = t.encode_utf16().collect();
            GdipDrawString(
                self.g,
                txt.as_ptr(),
                txt.len() as i32,
                font,
                &Rect { x, y, w, h },
                format,
                b,
            );
            GdipDeleteBrush(b);
            GdipDeleteStringFormat(format);
            GdipDeleteFont(font);
            GdipDeleteFontFamily(family);
        }
    }
    pub fn line(&self, x: f32, y: f32, x2: f32, y2: f32, c: u32, width: f32) {
        unsafe {
            let mut p = std::ptr::null_mut();
            GdipCreatePen1(c, width, 2, &mut p);
            GdipDrawLine(self.g, p, x, y, x2, y2);
            GdipDeletePen(p);
        }
    }
    pub fn circle(&self, x: f32, y: f32, r: f32, c: u32, edge: u32) {
        unsafe {
            let mut b = std::ptr::null_mut();
            GdipCreateSolidFill(c, &mut b);
            GdipFillEllipse(self.g, b, x - r, y - r, r * 2., r * 2.);
            GdipDeleteBrush(b);
            if edge != 0 {
                let mut p = std::ptr::null_mut();
                GdipCreatePen1(edge, 1.6, 2, &mut p);
                GdipDrawEllipse(self.g, p, x - r, y - r, r * 2., r * 2.);
                GdipDeletePen(p);
            }
        }
    }
    pub fn arc(&self, x: f32, y: f32, w: f32, h: f32, a: f32, b: f32, c: u32, width: f32) {
        unsafe {
            let mut p = std::ptr::null_mut();
            GdipCreatePen1(c, width, 2, &mut p);
            GdipDrawArc(self.g, p, x, y, w, h, a, b);
            GdipDeletePen(p);
        }
    }
    pub fn gradient(&self, r: Rect, c1: u32, c2: u32, vertical: bool) {
        unsafe {
            let mut b = std::ptr::null_mut();
            GdipCreateLineBrushFromRect(&r, c1, c2, if vertical { 1 } else { 0 }, 0, &mut b);
            GdipFillRectangle(self.g, b, r.x, r.y, r.w, r.h);
            GdipDeleteBrush(b);
        }
    }
    pub fn translate(&self, x: f32, y: f32) {
        unsafe {
            GdipTranslateWorldTransform(self.g, x, y, 0);
        }
    }
    pub fn close(&self, x: f32, y: f32) {
        self.line(x - 6., y - 6., x + 6., y + 6., TEXT, 1.3);
        self.line(x + 6., y - 6., x - 6., y + 6., TEXT, 1.3);
    }
    pub fn pen(&self, x: f32, y: f32) {
        self.line(x - 5., y + 5., x + 3., y - 3., TEXT, 3.);
        self.line(x + 5., y - 5., x + 8., y - 8., TEXT, 3.);
        self.line(x - 8., y + 8., x - 6., y + 6., TEXT, 1.2);
    }
    pub fn gear(&self, x: f32, y: f32) {
        // Six shallow, rounded teeth drawn as one thin continuous outline.
        let mut last = None;
        for i in 0..=96 {
            let a = i as f32 * std::f32::consts::TAU / 96.;
            let r = 9.3 + 1.1 * (6. * a).cos();
            let p = (x + r * a.cos(), y + r * a.sin());
            if let Some((px, py)) = last {
                self.line(px, py, p.0, p.1, TEXT, 1.35);
            }
            last = Some(p);
        }
        self.circle(x, y, 3.6, 0, TEXT);
    }
}
impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteGraphics(self.g);
        }
    }
}
pub fn hsv(h: f64, s: f64, v: f64) -> u32 {
    let c = v * s;
    let x = c * (1. - ((h / 60.) % 2. - 1.).abs());
    let m = v - c;
    let (r, g, b) = if h < 60. {
        (c, x, 0.)
    } else if h < 120. {
        (x, c, 0.)
    } else if h < 180. {
        (0., c, x)
    } else if h < 240. {
        (0., x, c)
    } else if h < 300. {
        (x, 0., c)
    } else {
        (c, 0., x)
    };
    (((r + m) * 255.).round() as u32) << 16
        | (((g + m) * 255.).round() as u32) << 8
        | ((b + m) * 255.).round() as u32
}
pub fn to_hsv(c: u32) -> (f64, f64, f64) {
    let r = ((c >> 16) & 255) as f64 / 255.;
    let g = ((c >> 8) & 255) as f64 / 255.;
    let b = (c & 255) as f64 / 255.;
    let max = r.max(g).max(b);
    let d = max - r.min(g).min(b);
    let h = if d == 0. {
        120.
    } else if max == r {
        60. * ((g - b) / d).rem_euclid(6.)
    } else if max == g {
        60. * ((b - r) / d + 2.)
    } else {
        60. * ((r - g) / d + 4.)
    };
    (h, if max == 0. { 0. } else { d / max }, max)
}
