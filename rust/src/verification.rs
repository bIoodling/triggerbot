//! Ignored, opt-in integration checks in Cargo's normal library test runner.
//! No separate test application is built or distributed.
use std::{
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::*,
    UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};
static DOWNS: AtomicU32 = AtomicU32::new(0);
static UPS: AtomicU32 = AtomicU32::new(0);
static PRESENT: AtomicBool = AtomicBool::new(false);
static TICK: AtomicU32 = AtomicU32::new(0);
static SCENE_COLOR: AtomicU32 = AtomicU32::new(COLOR);
static OWNED_KEYS: std::sync::Mutex<[bool; 256]> = std::sync::Mutex::new([false; 256]);
struct Session {
    shared: Arc<crate::trigger::Shared>,
    worker: Option<std::thread::JoinHandle<()>>,
    hwnd: HWND,
    instance: windows_sys::Win32::Foundation::HMODULE,
    class: Vec<u16>,
    cursor: POINT,
    foreground: HWND,
}
impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            self.shared.running.store(false, Ordering::Release);
            self.shared.wake.notify_all();
            if let Some(worker) = self.worker.take() {
                let _ = worker.join();
            }
            let mut keys = OWNED_KEYS.lock().unwrap_or_else(|p| p.into_inner());
            for (key, pressed) in keys.iter_mut().enumerate() {
                if *pressed {
                    let input = INPUT {
                        r#type: INPUT_KEYBOARD,
                        Anonymous: INPUT_0 {
                            ki: KEYBDINPUT {
                                wVk: key as u16,
                                dwFlags: KEYEVENTF_KEYUP,
                                dwExtraInfo: crate::input::TAG,
                                ..std::mem::zeroed()
                            },
                        },
                    };
                    SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
                    *pressed = false;
                }
            }
            let owns_focus = GetForegroundWindow() == self.hwnd;
            DestroyWindow(self.hwnd);
            UnregisterClassW(self.class.as_ptr(), self.instance);
            if owns_focus {
                SetCursorPos(self.cursor.x, self.cursor.y);
                if !self.foreground.is_null() {
                    SetForegroundWindow(self.foreground);
                }
            }
        }
    }
}
const COLOR: u32 = 0x28b4e0;
unsafe extern "system" fn scene(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let dc = BeginPaint(hwnd, &mut ps);
            let mut r: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut r);
            let bg = CreateSolidBrush(8 + (TICK.load(Ordering::Relaxed) & 1));
            FillRect(dc, &r, bg);
            DeleteObject(bg);
            if PRESENT.load(Ordering::Relaxed) {
                let patch = RECT {
                    left: r.right / 2 - 6,
                    top: r.bottom / 2 - 6,
                    right: r.right / 2 + 7,
                    bottom: r.bottom / 2 + 7,
                };
                let brush = CreateSolidBrush({
                    let color = SCENE_COLOR.load(Ordering::Relaxed);
                    ((color & 255) << 16) | (color & 0xff00) | ((color >> 16) & 255)
                });
                FillRect(dc, &patch, brush);
                DeleteObject(brush);
            }
            EndPaint(hwnd, &ps);
            0
        }
        WM_LBUTTONDOWN => {
            DOWNS.fetch_add(1, Ordering::Relaxed);
            0
        }
        WM_LBUTTONUP => {
            UPS.fetch_add(1, Ordering::Relaxed);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe fn pump(hwnd: HWND, ms: u64) {
    let until = Instant::now() + Duration::from_millis(ms);
    let mut next = Instant::now();
    while Instant::now() < until {
        let mut msg: MSG = std::mem::zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        if Instant::now() >= next {
            TICK.fetch_add(1, Ordering::Relaxed);
            InvalidateRect(hwnd, null(), 0);
            UpdateWindow(hwnd);
            next = Instant::now() + Duration::from_millis(8);
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
unsafe fn key(hwnd: HWND, v: u16, down: bool) {
    assert_eq!(
        GetForegroundWindow(),
        hwnd,
        "verification receiver lost focus; refusing input"
    );
    let i = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: v,
                dwFlags: if down { 0 } else { KEYEVENTF_KEYUP },
                dwExtraInfo: crate::input::TAG,
                ..std::mem::zeroed()
            },
        },
    };
    assert_eq!(SendInput(1, &i, std::mem::size_of::<INPUT>() as i32), 1);
    OWNED_KEYS.lock().unwrap()[v as usize] = down;
    pump(hwnd, 12);
}
#[test]
#[ignore = "requires an interactive Windows desktop; moves the cursor and emits F22/F23 plus left mouse clicks to its own receiver"]
fn capture_to_received_input_both_modes() {
    unsafe {
        DOWNS.store(0, Ordering::Relaxed);
        UPS.store(0, Ordering::Relaxed);
        for key in [87, 65, 83, 68, 133, 134] {
            assert!(
                !crate::input::down(key),
                "a verification key is already held; refusing interactive input"
            );
        }
        let foreground = GetForegroundWindow();
        let mut saved_cursor = POINT { x: 0, y: 0 };
        GetCursorPos(&mut saved_cursor);
        let instance = GetModuleHandleW(null());
        let name = crate::wide(&format!("YamiPipelineVerification{}", std::process::id()));
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            hInstance: instance,
            lpfnWndProc: Some(scene),
            lpszClassName: name.as_ptr(),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            name.as_ptr(),
            crate::wide("Yami controlled verification").as_ptr(),
            WS_POPUP,
            120,
            120,
            320,
            240,
            null_mut(),
            null_mut(),
            instance,
            null(),
        );
        assert!(!hwnd.is_null());
        let shared = crate::trigger::Shared::with_config(crate::config::Config::default());
        let mut session = Session {
            shared: shared.clone(),
            worker: None,
            hwnd,
            instance,
            class: name.clone(),
            cursor: saved_cursor,
            foreground,
        };
        ShowWindow(hwnd, SW_SHOW);
        SetForegroundWindow(hwnd);
        pump(hwnd, 40);
        SetCursorPos(280, 240);
        shared.change(|c| {
            c.config = crate::config::Config {
                toggle: 133,
                hold: 134,
                rgb: COLOR,
                tolerance: 3,
                radius: 12,
                ..Default::default()
            };
            c.enabled = true;
            c.suspended = false;
            c.held = false;
        });
        for movement in [87, 65, 83, 68] {
            key(hwnd, movement, true);
        }
        key(hwnd, 134, true);
        assert!(crate::input::down(134), "Windows Hold state is not down");
        session.worker = Some(crate::trigger::start(Arc::clone(&shared)));
        pump(hwnd, 150);
        PRESENT.store(true, Ordering::Relaxed);
        pump(hwnd, 400);
        let cursor_count = DOWNS.load(Ordering::Relaxed);
        eprintln!(
            "Cursor mode: received {cursor_count} down events, {} up events",
            UPS.load(Ordering::Relaxed)
        );
        key(hwnd, 134, false);
        PRESENT.store(false, Ordering::Relaxed);
        pump(hwnd, 60);
        shared.change(|c| {
            c.config.fixed = true;
            c.config.calibrated = true;
            c.config.x = 0.5;
            c.config.y = 0.5;
            c.config.target_exe = crate::capture::exe(hwnd);
            c.viewport = hwnd as usize;
        });
        key(hwnd, 134, true);
        pump(hwnd, 100);
        PRESENT.store(true, Ordering::Relaxed);
        pump(hwnd, 250);
        let fixed_count = DOWNS.load(Ordering::Relaxed) - cursor_count;
        PRESENT.store(false, Ordering::Relaxed);
        pump(hwnd, 75);
        PRESENT.store(true, Ordering::Relaxed);
        pump(hwnd, 200);
        let reappeared = DOWNS.load(Ordering::Relaxed);
        key(hwnd, 134, false);
        let after_release = DOWNS.load(Ordering::Relaxed);
        PRESENT.store(false, Ordering::Relaxed);
        pump(hwnd, 60);
        PRESENT.store(true, Ordering::Relaxed);
        pump(hwnd, 100);
        let release_ok = DOWNS.load(Ordering::Relaxed) == after_release;
        // Both a miss outside the tolerance and a near-colour match are sent
        // through real capture/Rust scanning/dispatch, not a CPU colour stub.
        shared.change(|c| {
            c.config.fixed = false;
            c.config.rgb = COLOR + 3;
            c.config.tolerance = 2;
        });
        key(hwnd, 134, true);
        pump(hwnd, 120);
        let narrow_miss = DOWNS.load(Ordering::Relaxed) == after_release;
        key(hwnd, 134, false);
        shared.change(|c| c.config.tolerance = 3);
        key(hwnd, 134, true);
        pump(hwnd, 120);
        let near_match = DOWNS.load(Ordering::Relaxed) == after_release + 1;
        key(hwnd, 134, false);
        // Standard input without a Raw Input event must still toggle permission
        // exactly once, even across several polling iterations.
        key(hwnd, 133, true);
        pump(hwnd, 25);
        key(hwnd, 133, false);
        let toggle_disabled = !shared.control.lock().unwrap().enabled;
        key(hwnd, 134, true);
        PRESENT.store(false, Ordering::Relaxed);
        pump(hwnd, 70);
        PRESENT.store(true, Ordering::Relaxed);
        pump(hwnd, 100);
        let disabled_count = DOWNS.load(Ordering::Relaxed);
        key(hwnd, 134, false);
        PRESENT.store(false, Ordering::Relaxed);
        key(hwnd, 133, true);
        pump(hwnd, 25);
        key(hwnd, 133, false);
        let toggle_enabled = shared.control.lock().unwrap().enabled;
        let mut colour_counts = Vec::new();
        for (index, color) in [
            0xff0000u32,
            0x00ff00,
            0x0000ff,
            0x7b2fff,
            0x123456,
            0xffffff,
        ]
        .into_iter()
        .enumerate()
        {
            SCENE_COLOR.store(color, Ordering::Relaxed);
            PRESENT.store(false, Ordering::Relaxed);
            pump(hwnd, 45);
            shared.change(|c| {
                c.config.rgb = color;
                c.config.tolerance = 2;
                c.config.fixed = index % 2 == 0;
            });
            key(hwnd, 134, true);
            pump(hwnd, 40);
            let before = DOWNS.load(Ordering::Relaxed);
            PRESENT.store(true, Ordering::Relaxed);
            pump(hwnd, 120);
            colour_counts.push(DOWNS.load(Ordering::Relaxed) - before);
            key(hwnd, 134, false);
        }
        shared.change(|c| {
            c.suspended = true;
            c.sample_request = Some((42, (280, 240)));
        });
        pump(hwnd, 100);
        let sampled = *shared.sampled_pixel.lock().unwrap();
        shared.control.lock().unwrap().sample_request = None;
        for movement in [87, 65, 83, 68] {
            key(hwnd, movement, false);
        }
        let total = DOWNS.load(Ordering::Relaxed);
        shared.running.store(false, Ordering::Release);
        shared.wake.notify_all();
        session.worker.take().unwrap().join().unwrap();
        eprintln!("Fixed Point: {fixed_count} initial events; first qualified appearances {reappeared}; immediately after Hold release {after_release}");
        assert_eq!(
            cursor_count, 1,
            "Cursor Hold/capture/scan/state/dispatch path failed"
        );
        assert_eq!(fixed_count, 1, "Fixed Point path failed");
        assert_eq!(
            reappeared, 3,
            "genuine reappearance was missed or duplicated"
        );
        assert_eq!(release_ok, true, "late dispatch after Hold release");
        assert!(
            narrow_miss && near_match,
            "RGB tolerance is not applied to native scan parameters"
        );
        assert!(
            toggle_disabled && toggle_enabled,
            "Toggle fallback did not produce one state transition per press"
        );
        assert_eq!(
            disabled_count,
            after_release + 1,
            "disabled state emitted input"
        );
        assert_eq!(
            colour_counts,
            vec![1, 1, 1, 1, 1, 1],
            "arbitrary colours failed in alternating modes"
        );
        assert_eq!(
            sampled.map(|(_, point, rgb)| (point, rgb)),
            Some(((280, 240), 0xffffff)),
            "eyedropper pixel did not come from real capture"
        );
        eprintln!("Verified {total} paired output events; tolerance boundary, Toggle, Hold release, 6 RGB colours, both modes and native eyedropper passed");
        assert_eq!(
            UPS.load(Ordering::Relaxed),
            DOWNS.load(Ordering::Relaxed),
            "unpaired output"
        );
    }
}
