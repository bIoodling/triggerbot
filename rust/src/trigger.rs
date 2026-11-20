use crate::{config::Config, input};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};
pub struct Control {
    pub config: Config,
    pub enabled: bool,
    pub held: bool,
    pub suspended: bool,
    pub generation: u64,
    pub viewport: usize,
    pub toggle_down: bool,
    pub require_hold_release: bool,
    pub ignore_toggle_until_up: bool,
    pub ui_hwnd: usize,
    pub sample_request: Option<(u64, (i32, i32))>,
}
pub struct Shared {
    pub control: Mutex<Control>,
    pub wake: Condvar,
    pub running: AtomicBool,
    pub status: AtomicU8,
    pub frames: AtomicU64,
    pub matches: AtomicU64,
    pub outputs: AtomicU64,
    pub observations: AtomicU64,
    pub last_iteration_to_input_us: AtomicU64,
    pub max_iteration_to_input_us: AtomicU64,
    pub last_error: Mutex<Option<String>>,
    pub sampled_pixel: Mutex<Option<(u64, (i32, i32), u32)>>,
}
impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            control: Mutex::new(Control {
                config: crate::config::load(),
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
            }),
            wake: Condvar::new(),
            running: AtomicBool::new(true),
            status: AtomicU8::new(2),
            frames: AtomicU64::new(0),
            matches: AtomicU64::new(0),
            outputs: AtomicU64::new(0),
            observations: AtomicU64::new(0),
            last_iteration_to_input_us: AtomicU64::new(0),
            max_iteration_to_input_us: AtomicU64::new(0),
            last_error: Mutex::new(None),
            sampled_pixel: Mutex::new(None),
        })
    }
    pub fn change(&self, f: impl FnOnce(&mut Control)) {
        let mut c = self.control.lock().unwrap();
        let previous = c.config.clone();
        let was_suspended = c.suspended;
        f(&mut c);
        if c.config != previous || c.suspended || was_suspended {
            c.toggle_down = input::down(c.config.toggle);
            c.require_hold_release = input::down(c.config.hold);
            c.held = false;
        }
        c.generation = c.generation.wrapping_add(1);
        self.wake.notify_all();
    }
    pub fn save(&self) {
        let c = self.control.lock().unwrap().config.clone();
        if let Err(e) = crate::config::save(&c) {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    crate::wide(&format!("Settings could not be saved: {e}")).as_ptr(),
                    crate::wide("Yami").as_ptr(),
                    0x10,
                );
            }
        }
    }
}
impl Shared {
    fn error(&self, message: String) {
        self.status.store(2, Ordering::Release);
        let mut previous = self.last_error.lock().unwrap();
        if previous.as_ref() != Some(&message) {
            unsafe {
                windows_sys::Win32::System::Diagnostics::Debug::OutputDebugStringW(
                    crate::wide(&format!("Yami: {message}\n")).as_ptr(),
                );
            }
            if let Some(directory) = crate::config::path().parent() {
                let _ = std::fs::create_dir_all(directory);
                use std::io::Write;
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(directory.join("engine.log"))
                {
                    let _ = writeln!(
                        file,
                        "{}: {message}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    );
                }
            }
            *previous = Some(message);
        }
    }
}
pub fn start(shared: Arc<Shared>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut gpu = None;
        let mut cached = Config::default();
        let mut cached_generation = 0;
        let mut cached_viewport = 0usize;
        let mut latch = crate::event::Latch::new();
        let clock = Instant::now();
        let mut generation = 0;
        // Initialization is independent of UI readiness and does not wait for
        // the first Hold press. Errors are reported instead of silently ignored.
        let initial_monitor = unsafe {
            let mut point = windows_sys::Win32::Foundation::POINT { x: 0, y: 0 };
            windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
            windows_sys::Win32::Graphics::Gdi::MonitorFromPoint(
                point,
                windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTOPRIMARY,
            )
        } as isize;
        match unsafe { crate::gpu::Detector::new(initial_monitor) } {
            Ok(detector) => {
                gpu = Some(detector);
                shared.status.store(0, Ordering::Release);
            }
            Err(error) => shared.error(format!("D3D11/DXGI initialization failed: {error:?}")),
        }
        while shared.running.load(Ordering::Acquire) {
            let sample = shared.control.lock().unwrap().sample_request;
            if let Some((id, point)) = sample {
                latch.reset(false);
                let monitor = unsafe {
                    windows_sys::Win32::Graphics::Gdi::MonitorFromPoint(
                        windows_sys::Win32::Foundation::POINT {
                            x: point.0,
                            y: point.1,
                        },
                        windows_sys::Win32::Graphics::Gdi::MONITOR_DEFAULTTONULL,
                    )
                } as isize;
                if monitor == 0 {
                    std::thread::sleep(Duration::from_millis(4));
                    continue;
                }
                if gpu
                    .as_ref()
                    .map(|g: &crate::gpu::Detector| g.monitor != monitor)
                    .unwrap_or(true)
                {
                    match unsafe { crate::gpu::Detector::new(monitor) } {
                        Ok(detector) => gpu = Some(detector),
                        Err(error) => {
                            shared.error(format!(
                                "Eyedropper capture initialization failed: {error:?}"
                            ));
                            std::thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    }
                }
                match unsafe { gpu.as_mut().unwrap().sample_pixel(point) } {
                    Ok(Some(rgb)) => {
                        if shared.control.lock().unwrap().sample_request == Some((id, point)) {
                            *shared.sampled_pixel.lock().unwrap() = Some((id, point, rgb));
                        }
                    }
                    Ok(None) => {
                        let mut pixel = shared.sampled_pixel.lock().unwrap();
                        if let Some((_, last_point, rgb)) = *pixel {
                            if last_point == point {
                                *pixel = Some((id, point, rgb));
                            }
                        }
                    }
                    Err(error) => {
                        *shared.sampled_pixel.lock().unwrap() = None;
                        gpu = None;
                        shared.error(format!("Eyedropper capture failed: {error:?}"));
                    }
                }
                continue;
            }
            let (version, viewport) = {
                let mut c = shared.control.lock().unwrap();
                // Raw Input wakes this worker immediately. A current-key-state
                // fallback also supports valid remote/remapped input that does
                // not create a Raw Input edge. Both paths use the same edge bit.
                if !c.suspended {
                    let toggle = c.config.toggle;
                    let state = input::down(toggle);
                    if c.ignore_toggle_until_up {
                        if !state {
                            c.ignore_toggle_until_up = false;
                        }
                        c.toggle_down = state;
                    } else {
                        input::route_key(&mut c, toggle, state);
                    }
                }
                let held = input::down(c.config.hold);
                if !held {
                    c.require_hold_release = false;
                    c.held = false;
                }
                let over_ui = input::pointer_over(c.ui_hwnd);
                if !c.enabled
                    || c.suspended
                    || !held
                    || c.require_hold_release
                    || over_ui
                    || c.config.hold == 0
                {
                    latch.reset(false);
                    shared.status.store(
                        if c.enabled && shared.last_error.lock().unwrap().is_none() {
                            if c.config.hold == 0 {
                                3
                            } else {
                                0
                            }
                        } else {
                            2
                        },
                        Ordering::Release,
                    );
                    let idle_ms = if !c.enabled || c.suspended || c.config.hold == 0 {
                        50
                    } else {
                        2
                    };
                    let _ = shared
                        .wake
                        .wait_timeout(c, Duration::from_millis(idle_ms))
                        .unwrap();
                    continue;
                }
                if cached_generation != c.generation {
                    cached = c.config.clone();
                    cached_generation = c.generation;
                    cached_viewport = c.viewport;
                }
                (c.generation, cached_viewport)
            };
            let cfg = &cached;
            if version != generation || latch.phase == crate::event::Phase::Idle {
                generation = version;
                latch.reset(true);
            }
            let Some((point, monitor)) = crate::capture::point(cfg, viewport) else {
                shared.status.store(4, Ordering::Release);
                latch.invalidate_clear();
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            if cfg.fixed {
                unsafe {
                    use windows_sys::Win32::UI::WindowsAndMessaging::*;
                    cached_viewport = GetAncestor(GetForegroundWindow(), GA_ROOT) as usize;
                }
            }
            if gpu
                .as_ref()
                .map(|g: &crate::gpu::Detector| g.monitor != monitor)
                .unwrap_or(true)
            {
                match unsafe { crate::gpu::Detector::new(monitor) } {
                    Ok(value) => {
                        gpu = Some(value);
                        *shared.last_error.lock().unwrap() = None;
                    }
                    Err(error) => {
                        shared.error(format!("D3D11/DXGI initialization failed: {error:?}"));
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
            }
            let iteration_started = Instant::now();
            let result = unsafe { gpu.as_mut().unwrap().frame(point, cfg) };
            let found = match result {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(error) => {
                    shared.error(format!("Desktop capture/GPU result failed: {error:?}"));
                    gpu = None;
                    latch.invalidate_clear();
                    continue;
                }
            };
            if gpu.as_ref().unwrap().last_fresh {
                shared.frames.fetch_add(1, Ordering::Relaxed);
            }
            let c = shared.control.lock().unwrap();
            if c.generation != version
                || !c.enabled
                || c.suspended
                || c.require_hold_release
                || !shared.running.load(Ordering::Acquire)
                || !input::down(cfg.hold)
            {
                latch.reset(false);
                continue;
            }
            // Fixed Point must still map to its intended foreground client.
            // Cursor observations must not be discarded merely because the
            // pointer moved during GPU processing; only guard our own UI here.
            if input::pointer_over(c.ui_hwnd)
                || (cfg.fixed
                    && crate::capture::point(cfg, cached_viewport) != Some((point, monitor)))
            {
                latch.invalidate_clear();
                continue;
            }
            shared.observations.fetch_add(1, Ordering::Relaxed);
            shared.status.store(1, Ordering::Release);
            if found {
                shared.matches.fetch_add(1, Ordering::Relaxed);
            }
            if latch.observe(
                found,
                clock.elapsed(),
                cfg.clear_frames,
                Duration::from_millis(cfg.clear_ms),
            ) {
                // Consume the event before attempting input; a failed dispatch
                // cannot create a burst of retries on a continuous highlight.
                if input::click() {
                    shared.outputs.fetch_add(1, Ordering::Relaxed);
                    *shared.last_error.lock().unwrap() = None;
                    let us = iteration_started
                        .elapsed()
                        .as_micros()
                        .min(u64::MAX as u128) as u64;
                    shared
                        .last_iteration_to_input_us
                        .store(us, Ordering::Relaxed);
                    shared
                        .max_iteration_to_input_us
                        .fetch_max(us, Ordering::Relaxed);
                } else {
                    shared.error("Windows did not accept the configured output pair; check target permissions and input support".into());
                }
                latch.dispatched();
            }
        }
        // Once per clean worker shutdown, never per frame. Counts describe this
        // process and Windows acceptance, not proof that a game consumed input.
        let c = shared.control.lock().unwrap();
        let report = serde_json::json!({
            "build": "1.0.6-click-repair",
            "target_rgb": crate::config::hex(c.config.rgb),
            "tolerance": c.config.tolerance, "radius": c.config.radius,
            "fixed": c.config.fixed, "calibrated": c.config.calibrated,
            "hold": crate::input::name(c.config.hold), "output": "Left mouse click",
            "fresh_frames": shared.frames.load(Ordering::Relaxed),
            "valid_observations": shared.observations.load(Ordering::Relaxed),
            "matching_observations": shared.matches.load(Ordering::Relaxed),
            "windows_accepted_pairs": shared.outputs.load(Ordering::Relaxed),
            "last_iteration_to_sendinput_us": shared.last_iteration_to_input_us.load(Ordering::Relaxed),
            "max_iteration_to_sendinput_us": shared.max_iteration_to_input_us.load(Ordering::Relaxed),
            "last_error": *shared.last_error.lock().unwrap(),
        });
        if let Some(parent) = crate::config::path().parent() {
            let _ = std::fs::create_dir_all(parent);
            if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
                let _ = std::fs::write(parent.join("engine-session.json"), bytes);
            }
        }
    })
}
