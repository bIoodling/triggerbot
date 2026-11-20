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
    if !binding_allowed(key) {
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
/// Binding keys are deliberately narrower than arbitrary Windows VK values.
pub fn binding_allowed(v: u16) -> bool {
    matches!(v, 5 | 6 | 8 | 9 | 13 | 16..=20 | 32..=40 | 45 | 46 | 48..=57 | 65..=90 | 96..=111 | 112..=135 | 144 | 145 | 160..=165 | 186..=192 | 219..=222 | 226)
}
pub fn binding_pair_allowed(key: u16, other: u16, held: impl Iterator<Item = u16>) -> bool {
    binding_allowed(key) && key != other && !held.filter(|v| *v != key).any(|v| v != 0)
}
pub fn left_click_pair() -> [INPUT; 2] {
    let down = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: MOUSEEVENTF_LEFTDOWN,
                dwExtraInfo: TAG,
                ..unsafe { std::mem::zeroed() }
            },
        },
    };
    let mut up = down;
    up.Anonymous.mi.dwFlags = MOUSEEVENTF_LEFTUP;
    [down, up]
}
pub(crate) fn dispatch_click(mut insert: impl FnMut(&[INPUT]) -> u32) -> bool {
    let pair = left_click_pair();
    let count = insert(&pair);
    if count == 1 && insert(&pair[1..]) != 1 {
        *PENDING_RELEASE.lock().unwrap() = Some(pair[1]);
    }
    count == 2
}
pub fn click() -> bool {
    release_pending();
    if PENDING_RELEASE.lock().unwrap().is_some() {
        return false;
    }
    dispatch_click(|inputs| unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        )
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_is_exactly_one_tagged_mouse_pair() {
        let mut calls = 0;
        assert!(dispatch_click(|inputs| {
            calls += 1;
            assert_eq!(inputs.len(), 2);
            unsafe {
                assert_eq!(inputs[0].r#type, INPUT_MOUSE);
                assert_eq!(inputs[0].Anonymous.mi.dwFlags, MOUSEEVENTF_LEFTDOWN);
                assert_eq!(inputs[1].Anonymous.mi.dwFlags, MOUSEEVENTF_LEFTUP);
                assert_eq!(inputs[0].Anonymous.mi.dwExtraInfo, TAG);
            }
            2
        }));
        assert_eq!(calls, 1);
    }
    #[test]
    fn partial_click_only_retries_release() {
        let mut calls = 0;
        assert!(!dispatch_click(|inputs| {
            calls += 1;
            if calls == 1 {
                assert_eq!(inputs.len(), 2);
                1
            } else {
                assert_eq!(inputs.len(), 1);
                unsafe {
                    assert_eq!(inputs[0].Anonymous.mi.dwFlags, MOUSEEVENTF_LEFTUP);
                }
                1
            }
        }));
        assert_eq!(calls, 2);
    }
    #[test]
    fn bindings_reject_duplicates_reserved_keys_and_chords() {
        for v in [0, 1, 2, 4, 91, 92, 93, 95, 255] {
            assert!(!binding_allowed(v));
        }
        for v in [5, 6, 65, 87, 160, 163, 112, 135, 186] {
            assert!(binding_allowed(v));
        }
        assert!(!binding_pair_allowed(69, 69, [].into_iter()));
        assert!(!binding_pair_allowed(69, 70, [162, 69].into_iter()));
        assert!(binding_pair_allowed(5, 69, [5].into_iter()));
    }
    #[test]
    fn ordinary_movement_keys_do_not_disarm_or_toggle_hold() {
        let mut c = crate::trigger::Control {
            config: crate::config::Config {
                hold: 69,
                toggle: 5,
                ..Default::default()
            },
            enabled: true,
            held: false,
            suspended: false,
            generation: 1,
            viewport: 0,
            toggle_down: false,
            require_hold_release: false,
            ignore_toggle_until_up: false,
            ui_hwnd: 0,
            sample_request: None,
        };
        route_key(&mut c, 69, true);
        let generation = c.generation;
        for key in [87, 65, 83, 68, 32] {
            route_key(&mut c, key, true);
            assert!(c.held);
            assert!(c.enabled);
            assert_eq!(c.generation, generation);
        }
        for key in [87, 65, 83, 68, 32] {
            route_key(&mut c, key, false);
            assert!(c.held);
        }
        route_key(&mut c, 69, false);
        assert!(!c.held);
        assert!(c.generation > generation);
        route_key(&mut c, 5, true);
        assert!(!c.enabled);
        route_key(&mut c, 5, true);
        assert!(!c.enabled);
        route_key(&mut c, 5, false);
        route_key(&mut c, 5, true);
        assert!(c.enabled);
    }
}
