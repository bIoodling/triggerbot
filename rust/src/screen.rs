//! Native AHK-style screen sampling: one bounded BitBlt, then an early-exit RGB scan.
//! No full-desktop readback, cached frame replay, GPU shader or per-pixel API calls.
use crate::config::Config;
use std::{
    io,
    ptr::null_mut,
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};
fn failure(operation: &str) -> io::Error {
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(0) {
        io::Error::other(format!("{operation} failed without a Win32 error code"))
    } else {
        io::Error::new(error.kind(), format!("{operation}: {error}"))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}
pub fn region(center: (i32, i32), radius: u32, bounds: (i32, i32, i32, i32)) -> Option<Region> {
    let r = radius.min(128) as i32;
    let left = center.0.saturating_sub(r).max(bounds.0);
    let top = center.1.saturating_sub(r).max(bounds.1);
    let right = center.0.saturating_add(r + 1).min(bounds.2);
    let bottom = center.1.saturating_add(r + 1).min(bounds.3);
    if center.0 < bounds.0
        || center.0 >= bounds.2
        || center.1 < bounds.1
        || center.1 >= bounds.3
        || right <= left
        || bottom <= top
    {
        return None;
    }
    Some(Region {
        left,
        top,
        width: right - left,
        height: bottom - top,
    })
}
/// Top-down, 32-bit BGRA DIB. Reused between scans, never shared across threads.
pub(crate) struct Dib {
    pub(crate) dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    pub(crate) bits: *mut u32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}
impl Dib {
    pub(crate) unsafe fn new(width: i32, height: i32) -> io::Result<Self> {
        if width <= 0 || height <= 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "empty capture bitmap",
            ));
        }
        let dc = CreateCompatibleDC(null_mut());
        if dc.is_null() {
            return Err(failure("CreateCompatibleDC"));
        }
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
        let mut data = null_mut();
        let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut data, null_mut(), 0);
        if bitmap.is_null() || data.is_null() {
            if !bitmap.is_null() {
                DeleteObject(bitmap);
            }
            DeleteDC(dc);
            return Err(failure("CreateDIBSection"));
        }
        let old = SelectObject(dc, bitmap);
        if old.is_null() || old as isize == -1 {
            DeleteObject(bitmap);
            DeleteDC(dc);
            return Err(failure("SelectObject"));
        }
        Ok(Self {
            dc,
            bitmap,
            old,
            bits: data.cast(),
            width,
            height,
        })
    }
    pub(crate) unsafe fn copy_from(
        &mut self,
        source: HDC,
        rect: Region,
        layered: bool,
    ) -> io::Result<()> {
        if rect.width <= 0
            || rect.height <= 0
            || rect.width > self.width
            || rect.height > self.height
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "ROI exceeds capture bitmap",
            ));
        }
        let rop = SRCCOPY | if layered { CAPTUREBLT } else { 0 } | NOMIRRORBITMAP;
        if BitBlt(
            self.dc,
            0,
            0,
            rect.width,
            rect.height,
            source,
            rect.left,
            rect.top,
            rop,
        ) == 0
        {
            return Err(failure("BitBlt"));
        }
        // The DIB pointer must not be read before queued GDI work completes.
        if GdiFlush() == 0 {
            return Err(failure("GdiFlush"));
        }
        Ok(())
    }
    pub(crate) unsafe fn pixels(&self) -> &[u32] {
        std::slice::from_raw_parts(self.bits, (self.width * self.height) as usize)
    }
}
impl Drop for Dib {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Scan {
    pub found: bool,
    pub captured: Instant,
    pub detected: Instant,
    pub capture_time: Duration,
    pub match_time: Duration,
    pub point: (i32, i32),
    pub pixels_examined: u32,
}
/// Returns immediately on the first inclusive per-channel RGB match. u32 pixels
/// from little-endian BGRA bytes have the numeric form 0xAARRGGBB.
pub fn find_colour(
    pixels: &[u32],
    stride: usize,
    width: usize,
    height: usize,
    rgb: u32,
    tolerance: u32,
) -> Option<u32> {
    if width == 0
        || height == 0
        || stride < width
        || pixels.len() < (height - 1).checked_mul(stride)?.checked_add(width)?
    {
        return None;
    }
    let red = ((rgb >> 16) & 255) as u8;
    let green = ((rgb >> 8) & 255) as u8;
    let blue = (rgb & 255) as u8;
    let tol = tolerance.min(255) as u8;
    let mut examined = 0;
    for y in 0..height {
        for &pixel in &pixels[y * stride..y * stride + width] {
            examined += 1;
            if ((pixel >> 16) as u8).abs_diff(red) <= tol
                && ((pixel >> 8) as u8).abs_diff(green) <= tol
                && (pixel as u8).abs_diff(blue) <= tol
            {
                return Some(examined);
            }
        }
    }
    None
}
pub struct Detector {
    desktop: HDC,
    dib: Dib,
}
impl Detector {
    pub unsafe fn new() -> io::Result<Self> {
        let desktop = GetDC(null_mut());
        if desktop.is_null() {
            return Err(failure("GetDC(screen)"));
        }
        match Dib::new(25, 25) {
            Ok(dib) => Ok(Self { desktop, dib }),
            Err(e) => {
                ReleaseDC(null_mut(), desktop);
                Err(e)
            }
        }
    }
    unsafe fn capture(
        &mut self,
        point: (i32, i32),
        radius: u32,
    ) -> io::Result<Option<(Region, Instant, Duration)>> {
        let monitor = MonitorFromPoint(
            POINT {
                x: point.0,
                y: point.1,
            },
            MONITOR_DEFAULTTONULL,
        );
        if monitor.is_null() {
            return Ok(None);
        }
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err(failure("GetMonitorInfoW"));
        }
        let b = info.rcMonitor;
        let Some(rect) = region(point, radius, (b.left, b.top, b.right, b.bottom)) else {
            return Ok(None);
        };
        let side = (radius.min(128) * 2 + 1) as i32;
        if self.dib.width < side || self.dib.height < side {
            self.dib = Dib::new(side, side)?;
        }
        let start = Instant::now();
        self.dib.copy_from(self.desktop, rect, true)?;
        let captured = Instant::now();
        Ok(Some((rect, captured, captured.duration_since(start))))
    }
    pub unsafe fn frame(
        &mut self,
        mut point: (i32, i32),
        cfg: &Config,
    ) -> io::Result<Option<Scan>> {
        if !cfg.fixed {
            let mut p = POINT { x: 0, y: 0 };
            if windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut p) == 0 {
                return Err(failure("GetCursorPos"));
            }
            point = (p.x, p.y);
        }
        let Some((rect, captured, capture_time)) = self.capture(point, cfg.radius)? else {
            return Ok(None);
        };
        let start = Instant::now();
        let hit = find_colour(
            self.dib.pixels(),
            self.dib.width as usize,
            rect.width as usize,
            rect.height as usize,
            cfg.rgb,
            cfg.tolerance,
        );
        let detected = Instant::now();
        Ok(Some(Scan {
            found: hit.is_some(),
            captured,
            detected,
            capture_time,
            match_time: detected.duration_since(start),
            point,
            pixels_examined: hit.unwrap_or((rect.width * rect.height) as u32),
        }))
    }
    pub unsafe fn sample_pixel(&mut self, point: (i32, i32)) -> io::Result<Option<u32>> {
        if self.capture(point, 0)?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.dib.pixels()[0] & 0xffffff))
    }
}
impl Drop for Detector {
    fn drop(&mut self) {
        unsafe {
            ReleaseDC(null_mut(), self.desktop);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    unsafe fn fill(dib: &mut Dib, rgb: u32) {
        GdiFlush();
        std::slice::from_raw_parts_mut(dib.bits, (dib.width * dib.height) as usize)
            .fill(0xff000000 | rgb);
    }
    unsafe fn read_roi(
        source: &Dib,
        dest: &mut Dib,
        rect: Region,
        target: u32,
        tol: u32,
    ) -> io::Result<Option<u32>> {
        dest.copy_from(source.dc, rect, true)?;
        Ok(find_colour(
            dest.pixels(),
            dest.width as usize,
            rect.width as usize,
            rect.height as usize,
            target,
            tol,
        ))
    }
    #[test]
    fn bulk_capture_every_rgb_and_tolerance_including_bgra_order() {
        unsafe {
            let mut source = Dib::new(25, 25).unwrap();
            let mut dest = Dib::new(25, 25).unwrap();
            let rect = Region {
                left: 0,
                top: 0,
                width: 25,
                height: 25,
            };
            for rgb in [
                0xff0000, 0x00ff00, 0x0000ff, 0xffffff, 0x000000, 0x7b2fff, 0xff00ff, 0x123456,
            ] {
                fill(&mut source, !rgb & 0xffffff);
                *source.bits.add(624) = 0xff000000 | rgb;
                assert_eq!(
                    read_roi(&source, &mut dest, rect, rgb, 0).unwrap(),
                    Some(625),
                    "colour #{rgb:06X}"
                );
            }
            fill(&mut source, 0x332211);
            assert_eq!(
                read_roi(&source, &mut dest, rect, 0x112233, 8).unwrap(),
                None,
                "channel swap"
            );
            fill(&mut source, 0x406088);
            assert_eq!(
                read_roi(&source, &mut dest, rect, 0x406080, 8).unwrap(),
                Some(1)
            );
            fill(&mut source, 0x406089);
            assert_eq!(
                read_roi(&source, &mut dest, rect, 0x406080, 8).unwrap(),
                None
            );
        }
    }
    #[test]
    fn square_corners_early_exit_and_clipped_stride_have_no_stale_pixels() {
        unsafe {
            let mut source = Dib::new(25, 25).unwrap();
            let mut dest = Dib::new(25, 25).unwrap();
            let rect = region((12, 12), 12, (0, 0, 25, 25)).unwrap();
            assert_eq!((rect.width, rect.height), (25, 25));
            fill(&mut source, 0);
            *source.bits = 0xff123456;
            assert_eq!(
                read_roi(&source, &mut dest, rect, 0x123456, 0).unwrap(),
                Some(1),
                "AHK square corner must be included"
            );
            fill(&mut source, 0xffffff);
            let small = Region {
                left: 0,
                top: 0,
                width: 1,
                height: 1,
            };
            assert_eq!(
                read_roi(&source, &mut dest, small, 0x123456, 0).unwrap(),
                None,
                "old padding must not be scanned"
            );
            assert!(
                dest.copy_from(null_mut(), rect, true).is_err(),
                "a failed capture must not return an old match"
            );
        }
    }
    #[test]
    fn cursor_and_fixed_centres_use_identical_capture_match_and_click_logic() {
        unsafe {
            let mut source = Dib::new(80, 80).unwrap();
            let mut dest = Dib::new(25, 25).unwrap();
            for center in [(20, 20), (55, 55)] {
                let rect = region(center, 12, (0, 0, 80, 80)).unwrap();
                let mut latch = crate::event::Latch::new();
                latch.reset(true);
                let mut clicks = 0;
                for (t, present) in [
                    (0, false),
                    (1, true),
                    (2, true),
                    (3, false),
                    (4, true),
                    (5, true),
                    (6, false),
                    (7, false),
                    (8, true),
                    (9, true),
                ] {
                    fill(&mut source, 0);
                    if present {
                        *source.bits.add((center.1 * 80 + center.0) as usize) = 0xff7b2fff;
                    }
                    let found = read_roi(&source, &mut dest, rect, 0x7b2fff, 8)
                        .unwrap()
                        .is_some();
                    if latch.observe(found, Duration::from_millis(t), 2, Duration::ZERO) {
                        latch.begin_click();
                        assert!(crate::input::dispatch_click(|pair| {
                            assert_eq!(pair.len(), 2);
                            clicks += 1;
                            2
                        }));
                        latch.dispatched();
                    }
                }
                assert_eq!(
                    clicks, 2,
                    "flicker or sustained match must not double click at {center:?}"
                );
            }
        }
    }
    #[test]
    fn negative_monitor_coordinates_and_edges_are_clipped_in_physical_pixels() {
        assert_eq!(
            region((-1920, 0), 12, (-1920, 0, 0, 1080)),
            Some(Region {
                left: -1920,
                top: 0,
                width: 13,
                height: 13
            })
        );
        assert_eq!(
            region((1919, 1079), 12, (0, 0, 1920, 1080)),
            Some(Region {
                left: 1907,
                top: 1067,
                width: 13,
                height: 13
            })
        );
        assert_eq!(region((1920, 100), 12, (0, 0, 1920, 1080)), None);
    }
    #[test]
    fn memory_dc_latency_measurements_are_not_desktop_or_sendinput_measurements() {
        unsafe {
            let mut source = Dib::new(25, 25).unwrap();
            let mut dest = Dib::new(25, 25).unwrap();
            fill(&mut source, 0x12344d);
            *source.bits.add(624) = 0xff123456;
            let rect = Region {
                left: 0,
                top: 0,
                width: 25,
                height: 25,
            };
            let mut copies = Vec::with_capacity(5000);
            let mut matches = Vec::with_capacity(5000);
            let mut sinks = Vec::with_capacity(5000);
            for _ in 0..5100 {
                let start = Instant::now();
                dest.copy_from(source.dc, rect, true).unwrap();
                let copied = Instant::now();
                assert!(find_colour(dest.pixels(), 25, 25, 25, 0x123456, 8).is_some());
                let detected = Instant::now();
                let mut called = detected;
                assert!(crate::input::dispatch_click(|_| {
                    called = Instant::now();
                    2
                }));
                if copies.len() < 5000 {
                    copies.push(copied.duration_since(start).as_nanos() as u64);
                    matches.push(detected.duration_since(copied).as_nanos() as u64);
                    sinks.push(called.duration_since(detected).as_nanos() as u64);
                }
            }
            for (name, mut values) in [
                ("offscreen_BitBlt_GdiFlush", copies),
                ("Rust_625_pixel_search", matches),
            ] {
                values.sort_unstable();
                eprintln!("HEADLESS {name}: samples={} median_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",values.len(),values[2500] as f64/1000.,values[4750] as f64/1000.,values[4950] as f64/1000.,values[4999] as f64/1000.);
            }
        }
    }
}
