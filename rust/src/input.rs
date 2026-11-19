use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
pub const TAG: usize = 0x59414d49;
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
            SendInput(1, &b, std::mem::size_of::<INPUT>() as i32);
        }
        n == 2
    }
}
