use crate::config::Config;
use windows_sys::Win32::{
    Foundation::*, Graphics::Gdi::*, System::Threading::*, UI::WindowsAndMessaging::*,
};
pub unsafe fn exe(hwnd: HWND) -> String {
    let mut pid = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if p.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 1024];
    let mut n = 1024;
    let ok = QueryFullProcessImageNameW(p, 0, buf.as_mut_ptr(), &mut n);
    CloseHandle(p);
    if ok == 0 {
        return String::new();
    }
    std::path::Path::new(&String::from_utf16_lossy(&buf[..n as usize]))
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase()
}
pub fn point(cfg: &Config, hint: usize) -> Option<((i32, i32), isize)> {
    unsafe {
        let p = if cfg.fixed {
            if !cfg.calibrated {
                return None;
            }
            let foreground = GetAncestor(GetForegroundWindow(), GA_ROOT);
            let window = if hint != 0 && foreground == hint as HWND {
                foreground
            } else if exe(foreground) == cfg.target_exe {
                foreground
            } else {
                return None;
            };
            if window.is_null() || IsIconic(window) != 0 || IsWindowVisible(window) == 0 {
                return None;
            }
            let mut r: RECT = std::mem::zeroed();
            if GetClientRect(window, &mut r) == 0 {
                return None;
            }
            if r.right <= 0 || r.bottom <= 0 {
                return None;
            }
            let mut origin = POINT { x: 0, y: 0 };
            if ClientToScreen(window, &mut origin) == 0 {
                return None;
            }
            POINT {
                x: origin.x + ((cfg.x * r.right as f64).round() as i32).clamp(0, r.right - 1),
                y: origin.y + ((cfg.y * r.bottom as f64).round() as i32).clamp(0, r.bottom - 1),
            }
        } else {
            let mut p = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut p) == 0 {
                return None;
            }
            p
        };
        let monitor = MonitorFromPoint(p, MONITOR_DEFAULTTONULL);
        if monitor.is_null() {
            None
        } else {
            Some(((p.x, p.y), monitor as isize))
        }
    }
}
