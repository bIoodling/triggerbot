use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub const TAG: usize = 0x59414d49;
static PENDING_RELEASE: std::sync::Mutex<Option<INPUT>> = std::sync::Mutex::new(None);
pub fn release_pending() {
    unsafe {
        let mut pending = PENDING_RELEASE.lock().unwrap();
        if let Some(input) = pending.as_ref() {
            if SendInput(1, input, std::mem::size_of::<INPUT>() as i32) == 1 {
                *pending = None;
            }
        }
    }
}
pub fn route_key(c: &mut crate::trigger::Control, key: u16, pressed: bool) {
    if key == 0 || key > 254 {
        return;
    }
    if key == c.config.toggle {
        if pressed && !c.toggle_down && !c.suspended && !c.ignore_toggle_until_up {
            c.enabled = !c.enabled;
            c.generation = c.generation.wrapping_add(1);
        }
        c.toggle_down = pressed;
    }
    if key == c.config.hold {
        let was_held = c.held;
        let was_waiting = c.require_hold_release;
        c.held = pressed;
        if !pressed {
            c.require_hold_release = false;
            if was_held || was_waiting {
                c.generation = c.generation.wrapping_add(1);
            }
        }
    }
}
pub fn pointer_over(hwnd: usize) -> bool {
    if hwnd == 0 {
        return false;
    }
    unsafe {
        use windows_sys::Win32::{Foundation::POINT, UI::WindowsAndMessaging::*};
        let mut p = POINT { x: 0, y: 0 };
        GetCursorPos(&mut p);
        GetAncestor(WindowFromPoint(p), GA_ROOT) == hwnd as _
    }
}
pub fn down(v: u16) -> bool {
    v > 0 && v <= 254 && unsafe { GetAsyncKeyState(v as i32) as u16 & 0x8000 != 0 }
}
pub fn name(v: u16) -> String {
    match v {
        0 => "Not set".into(),
        1 => "Left Mouse".into(),
        2 => "Right Mouse".into(),
        4 => "Middle Mouse".into(),
        5 => "Mouse 4".into(),
        6 => "Mouse 5".into(),
        160 => "Left Shift".into(),
        161 => "Right Shift".into(),
        162 => "Left Ctrl".into(),
        163 => "Right Ctrl".into(),
        164 => "Left Alt".into(),
        165 => "Right Alt".into(),
        _ => unsafe {
            let scan = MapVirtualKeyW(v as u32, MAPVK_VK_TO_VSC_EX);
            let bits = ((scan & 255) << 16) | if scan & 0xff00 != 0 { 1 << 24 } else { 0 };
            let mut buf = [0u16; 64];
            let n = GetKeyNameTextW(bits as i32, buf.as_mut_ptr(), 64);
            if n > 0 {
                String::from_utf16_lossy(&buf[..n as usize])
            } else {
                "Unknown key".into()
            }
        },
    }
}
pub fn send(v: u16) -> bool {
    if v == 0 || v > 254 {
        return false;
    }
    unsafe {
        let mut a: INPUT = std::mem::zeroed();
        let mut b: INPUT;
        if [1, 2, 4, 5, 6].contains(&v) {
            a.r#type = INPUT_MOUSE;
            let (d, u, x) = match v {
                1 => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP, 0),
                2 => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, 0),
                4 => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, 0),
                5 => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 1),
                _ => (MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, 2),
            };
            a.Anonymous.mi = MOUSEINPUT {
                dwFlags: d,
                mouseData: x,
                dwExtraInfo: TAG,
                ..std::mem::zeroed()
            };
            b = a;
            b.Anonymous.mi.dwFlags = u;
        } else {
            a.r#type = INPUT_KEYBOARD;
            let scan = MapVirtualKeyW(v as u32, MAPVK_VK_TO_VSC_EX);
            a.Anonymous.ki = KEYBDINPUT {
                wVk: if scan == 0 { v } else { 0 },
                wScan: (scan & 255) as u16,
                dwFlags: if scan == 0 {
                    0
                } else {
                    KEYEVENTF_SCANCODE
                        | if scan & 0xff00 == 0xe000 {
                            KEYEVENTF_EXTENDEDKEY
                        } else {
                            0
                        }
                },
                dwExtraInfo: TAG,
                ..std::mem::zeroed()
            };
            b = a;
            b.Anonymous.ki.dwFlags |= KEYEVENTF_KEYUP;
        }
        let pair = [a, b];
        let n = SendInput(2, pair.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        if n == 1 {
            if SendInput(1, &b, std::mem::size_of::<INPUT>() as i32) != 1 {
                *PENDING_RELEASE.lock().unwrap() = Some(b);
            }
        }
        n == 2
    }
}
