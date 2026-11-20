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
            let point =
                normalized_point((origin.x, origin.y), (r.right, r.bottom), (cfg.x, cfg.y))?;
            POINT {
                x: point.0,
                y: point.1,
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

pub fn normalized_point(
    origin: (i32, i32),
    size: (i32, i32),
    normalized: (f64, f64),
) -> Option<(i32, i32)> {
    if size.0 <= 0 || size.1 <= 0 || !normalized.0.is_finite() || !normalized.1.is_finite() {
        return None;
    }
    Some((
        origin.0
            + ((normalized.0.clamp(0., 1.) * size.0 as f64).round() as i32).clamp(0, size.0 - 1),
        origin.1
            + ((normalized.1.clamp(0., 1.) * size.1 as f64).round() as i32).clamp(0, size.1 - 1),
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_point_follows_physical_client_bounds_at_dpi_scales_and_after_window_moves() {
        for percent in [100, 125, 150, 200] {
            let size = (800 * percent / 100, 600 * percent / 100);
            assert_eq!(
                normalized_point((-1600, 40), size, (0.5, 0.25)),
                Some((-1600 + size.0 / 2, 40 + (size.1 + 2) / 4))
            );
        }
        assert_eq!(
            normalized_point((200, 100), (1920, 1080), (1., 1.)),
            Some((2119, 1179))
        );
        assert_eq!(normalized_point((0, 0), (0, 1080), (0.5, 0.5)), None);
    }
}
