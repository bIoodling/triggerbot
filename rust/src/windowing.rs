//! Native window placement and one-shot, time-based launch fading.
use std::{ptr::null_mut, time::Instant};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};
pub unsafe fn active_monitor() -> HMONITOR {
    let mut p = POINT { x: 0, y: 0 };
    GetCursorPos(&mut p);
    MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST)
}
pub unsafe fn work_area(monitor: HMONITOR) -> RECT {
    let mut info: MONITORINFO = std::mem::zeroed();
    info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if GetMonitorInfoW(monitor, &mut info) == 0 {
        SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            (&mut info.rcWork as *mut RECT).cast(),
            0,
        );
    }
    info.rcWork
}
pub unsafe fn center(hwnd: HWND, monitor: HMONITOR) {
    let work = work_area(monitor);
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(hwnd, &mut r);
    let w = r.right - r.left;
    let h = r.bottom - r.top;
    let x = work.left + ((work.right - work.left - w).max(0)) / 2;
    let y = work.top + ((work.bottom - work.top - h).max(0)) / 2;
    SetWindowPos(
        hwnd,
        null_mut(),
        x,
        y,
        0,
        0,
        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
    );
}
pub unsafe fn centered_dialog(parent: HWND, w: i32, h: i32) -> (i32, i32) {
    let mut r: RECT = std::mem::zeroed();
    GetWindowRect(parent, &mut r);
    let work = work_area(MonitorFromWindow(parent, MONITOR_DEFAULTTONEAREST));
    let x = (r.left + (r.right - r.left - w) / 2).clamp(work.left, (work.right - w).max(work.left));
    let y = (r.top + (r.bottom - r.top - h) / 2).clamp(work.top, (work.bottom - h).max(work.top));
    (x, y)
}
pub struct Fade {
    start: Option<Instant>,
}
impl Fade {
    pub fn new() -> Self {
        Self { start: None }
    }
    pub unsafe fn show(&mut self, hwnd: HWND) {
        let animate = animations_enabled();
        SetLayeredWindowAttributes(hwnd, 0, if animate { 0 } else { 255 }, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        if animate {
            self.start = Some(Instant::now());
            SetTimer(hwnd, 2, 16, None);
        }
    }
    pub unsafe fn tick(&mut self, hwnd: HWND) {
        if let Some(start) = self.start {
            let t = (start.elapsed().as_secs_f64() / 0.2).clamp(0., 1.);
            let alpha = ((1. - (1. - t).powi(3)) * 255.).round() as u8;
            SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
            if t >= 1. {
                self.start = None;
                KillTimer(hwnd, 2);
            }
        }
    }
}

pub unsafe fn animations_enabled() -> bool {
    let mut enabled = 0i32;
    let ok = SystemParametersInfoW(
        SPI_GETCLIENTAREAANIMATION,
        0,
        (&mut enabled as *mut i32).cast(),
        0,
    ) != 0;
    let mut animation = ANIMATIONINFO {
        cbSize: std::mem::size_of::<ANIMATIONINFO>() as u32,
        iMinAnimate: 0,
    };
    let animation_ok = SystemParametersInfoW(
        SPI_GETANIMATION,
        animation.cbSize,
        (&mut animation as *mut ANIMATIONINFO).cast(),
        0,
    ) != 0;
    ok && enabled != 0 && animation_ok && animation.iMinAnimate != 0
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmFlush() -> i32;
}
/// Wait for the hide transaction before requesting a screen colour sample.
pub unsafe fn flush_hidden_windows() {
    let _ = DwmFlush();
}
