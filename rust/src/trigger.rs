use crate::{config::Config, input};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering},
    Arc, Condvar, Mutex,
};
use std::time::{Duration, Instant};
static WORKER_ACTIVE: AtomicBool = AtomicBool::new(false);
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
    pub worker_started: AtomicBool,
    pub capture_time: crate::metrics::Timing,
    pub match_time: crate::metrics::Timing,
    pub match_to_sendinput: crate::metrics::Timing,
    pub sendinput_duration: crate::metrics::Timing,
    pub last_iteration_to_input_us: AtomicU64,
    pub max_iteration_to_input_us: AtomicU64,
    pub last_error: Mutex<Option<String>>,
    pub sampled_pixel: Mutex<Option<(u64, (i32, i32), u32)>>,
}
impl Shared {
    pub fn new() -> Arc<Self> {
        Self::with_config(crate::config::load())
    }
    pub fn with_config(config: Config) -> Arc<Self> {
        Arc::new(Self {
            control: Mutex::new(Control {
                config: crate::config::normalize(config),
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
            worker_started: AtomicBool::new(false),
            capture_time: crate::metrics::Timing::new(),
            match_time: crate::metrics::Timing::new(),
            match_to_sendinput: crate::metrics::Timing::new(),
            sendinput_duration: crate::metrics::Timing::new(),
            last_iteration_to_input_us: AtomicU64::new(0),
            max_iteration_to_input_us: AtomicU64::new(0),
            last_error: Mutex::new(None),
            sampled_pixel: Mutex::new(None),
        })
    }
    pub(crate) fn claim_worker(&self) -> bool {
        self.worker_started
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }
    pub fn change(&self, f: impl FnOnce(&mut Control)) {
        let mut c = self.control.lock().unwrap();
        let previous = c.config.clone();
        let was_suspended = c.suspended;
        f(&mut c);
        if c.config.toggle == 0 && previous.toggle != 0 {
            c.enabled = true;
        }
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
        if WORKER_ACTIVE.swap(true, Ordering::AcqRel) {
            return;
        }
        if !shared.claim_worker() {
            WORKER_ACTIVE.store(false, Ordering::Release);
            return;
        }
        struct Lease(std::sync::Arc<Shared>);
        impl Drop for Lease {
            fn drop(&mut self) {
                self.0.worker_started.store(false, Ordering::Release);
                WORKER_ACTIVE.store(false, Ordering::Release);
            }
        }
        let _lease = Lease(shared.clone());
        let pacer = crate::pacing::Pacer::new();
        let mut previous_scan = None;
        let mut detector = None;
        let mut cached = Config::default();
        let mut cached_generation = 0;
        let mut cached_viewport = 0usize;
        let mut latch = crate::event::Latch::new();
        let clock = Instant::now();
        let mut generation = 0;
        // Initialization is independent of UI readiness and does not wait for
        // the first Hold press. Errors are reported instead of silently ignored.
        match unsafe { crate::screen::Detector::new() } {
            Ok(created) => {
                detector = Some(created);
                shared.status.store(0, Ordering::Release);
            }
            Err(error) => shared.error(format!("Screen capture initialization failed: {error:?}")),
        }
        while shared.running.load(Ordering::Acquire) {
            let sample = shared.control.lock().unwrap().sample_request;
            if let Some((id, point)) = sample {
                previous_scan = None;
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
                if detector.is_none() {
                    match unsafe { crate::screen::Detector::new() } {
                        Ok(created) => detector = Some(created),
                        Err(error) => {
                            shared.error(format!(
                                "Eyedropper capture initialization failed: {error:?}"
                            ));
                            std::thread::sleep(Duration::from_millis(50));
                            continue;
                        }
                    }
                }
                match unsafe { detector.as_mut().unwrap().sample_pixel(point) } {
                    Ok(Some(rgb)) => {
                        if shared.control.lock().unwrap().sample_request == Some((id, point)) {
                            *shared.sampled_pixel.lock().unwrap() = Some((id, point, rgb));
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        *shared.sampled_pixel.lock().unwrap() = None;
                        detector = None;
                        shared.error(format!("Eyedropper capture failed: {error:?}"));
                    }
                }
                let c = shared.control.lock().unwrap();
                let _ = shared.wake.wait_timeout(c, Duration::from_millis(8));
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
                if !eligible(&c, held) || over_ui {
                    previous_scan = None;
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
                previous_scan = None;
                let c = shared.control.lock().unwrap();
                let _ = shared.wake.wait_timeout(c, Duration::from_millis(2));
                continue;
            };
            if cfg.fixed {
                unsafe {
                    use windows_sys::Win32::UI::WindowsAndMessaging::*;
                    cached_viewport = GetAncestor(GetForegroundWindow(), GA_ROOT) as usize;
                }
            }
            if detector.is_none() {
                match unsafe { crate::screen::Detector::new() } {
                    Ok(value) => {
                        detector = Some(value);
                        *shared.last_error.lock().unwrap() = None;
                    }
                    Err(error) => {
                        shared.error(format!("Screen capture initialization failed: {error:?}"));
                        std::thread::sleep(Duration::from_millis(100));
                        continue;
                    }
                }
            }
            if let Some(previous) = previous_scan {
                pacer.wait_since(previous);
            }
            let iteration_started = Instant::now();
            previous_scan = Some(iteration_started);
            let result = unsafe { detector.as_mut().unwrap().frame(point, cfg) };
            let scan = match result {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(error) => {
                    shared.error(format!("Screen capture/pixel scan failed: {error:?}"));
                    detector = None;
                    latch.invalidate_clear();
                    previous_scan = None;
                    let c = shared.control.lock().unwrap();
                    let _ = shared.wake.wait_timeout(c, Duration::from_millis(20));
                    continue;
                }
            };
            let found = scan.found;
            let c = shared.control.lock().unwrap();
            if !dispatch_allowed(
                &c,
                version,
                shared.running.load(Ordering::Acquire),
                input::down(cfg.hold),
            ) {
                latch.reset(false);
                continue;
            }
            // Fixed Point must still map to its intended foreground client.
            // Cursor observations must not be discarded merely because the
            // pointer moved during capture/pixel processing; only guard our own UI here.
            if input::pointer_over(c.ui_hwnd)
                || input::point_over(c.ui_hwnd, scan.point)
                || (cfg.fixed
                    && crate::capture::point(cfg, cached_viewport) != Some((point, monitor)))
            {
                latch.invalidate_clear();
                continue;
            }
            shared.status.store(1, Ordering::Release);
            if latch.observe(
                found,
                clock.elapsed(),
                cfg.clear_frames,
                Duration::from_millis(cfg.clear_ms),
            ) {
                // Consume the event before attempting input; a failed dispatch
                // cannot create a burst of retries on a continuous highlight.
                latch.begin_click();
                let timing = input::click_timed();
                if let Some(ref timing) = timing {
                    shared
                        .match_to_sendinput
                        .record(timing.called.saturating_duration_since(scan.detected));
                    shared
                        .sendinput_duration
                        .record(timing.returned.saturating_duration_since(timing.called));
                }
                if timing.as_ref().map(|t| t.accepted).unwrap_or(false) {
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
            drop(c);
            shared.frames.fetch_add(1, Ordering::Relaxed);
            shared.observations.fetch_add(1, Ordering::Relaxed);
            if found {
                shared.matches.fetch_add(1, Ordering::Relaxed);
            }
            shared.capture_time.record(scan.capture_time);
            shared.match_time.record(scan.match_time);
        }
        // Once per clean worker shutdown, never per frame. Counts describe this
        // process and Windows acceptance, not proof that a game consumed input.
        let c = shared.control.lock().unwrap();
        let report = serde_json::json!({
            "build": "1.0.6-native-ahk-scan",
            "target_rgb": crate::config::hex(c.config.rgb),
            "tolerance": c.config.tolerance, "radius": c.config.radius,
            "fixed": c.config.fixed, "calibrated": c.config.calibrated,
            "hold": crate::input::name(c.config.hold), "output": "Left mouse click",
            "fresh_screen_samples": shared.frames.load(Ordering::Relaxed),
            "capture_copy_and_gdi_flush": shared.capture_time.report(),
            "rust_pixel_matching": shared.match_time.report(),
            "confirmed_match_to_sendinput_entry": shared.match_to_sendinput.report(),
            "sendinput_call_duration": shared.sendinput_duration.report(),
            "display_refresh_latency": "not measured; GDI exposes no presentation timestamps",
            "target_exe":c.config.target_exe, "normalized_point":[c.config.x,c.config.y],
            "valid_observations": shared.observations.load(Ordering::Relaxed),
            "matching_observations": shared.matches.load(Ordering::Relaxed),
            "windows_accepted_pairs": shared.outputs.load(Ordering::Relaxed),
            "last_iteration_to_sendinput_us": shared.last_iteration_to_input_us.load(Ordering::Relaxed),
            "max_iteration_to_sendinput_us": shared.max_iteration_to_input_us.load(Ordering::Relaxed),
            "last_error": *shared.last_error.lock().unwrap(),
        });
        if cfg!(test) {
            eprintln!("CONTROLLED_ENGINE_TIMINGS {report}");
            return;
        }
        if let Some(parent) = crate::config::path().parent() {
            let _ = std::fs::create_dir_all(parent);
            if let Ok(bytes) = serde_json::to_vec_pretty(&report) {
                let _ = std::fs::write(parent.join("engine-session.json"), bytes);
            }
        }
    })
}

pub(crate) fn eligible(c: &Control, hold_pressed: bool) -> bool {
    c.enabled && !c.suspended && !c.require_hold_release && c.config.hold != 0 && hold_pressed
}
pub(crate) fn dispatch_allowed(
    c: &Control,
    generation: u64,
    running: bool,
    hold_pressed: bool,
) -> bool {
    running && c.generation == generation && eligible(c, hold_pressed)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hold_toggle_thumb_buttons_wasd_and_stale_generation_share_one_gate() {
        let shared = Shared::with_config(Config {
            hold: 6,
            toggle: 5,
            ..Default::default()
        });
        let mut c = shared.control.lock().unwrap();
        assert!(!eligible(&c, false));
        input::route_key(&mut c, 6, true);
        assert!(eligible(&c, true));
        let generation = c.generation;
        for key in [87, 65, 83, 68] {
            input::route_key(&mut c, key, true);
            assert!(dispatch_allowed(&c, generation, true, true));
        }
        assert!(!dispatch_allowed(&c, generation, true, false));
        input::route_key(&mut c, 6, false);
        assert!(!dispatch_allowed(&c, generation, true, true));
        input::route_key(&mut c, 6, true);
        input::route_key(&mut c, 5, true);
        assert!(!eligible(&c, true));
        input::route_key(&mut c, 5, true);
        assert!(!eligible(&c, true));
        input::route_key(&mut c, 5, false);
        input::route_key(&mut c, 5, true);
        assert!(eligible(&c, true));
        c.suspended = true;
        assert!(!eligible(&c, true));
        c.suspended = false;
        c.require_hold_release = true;
        assert!(!eligible(&c, true));
        assert!(!dispatch_allowed(&c, c.generation, false, true));
    }
    #[test]
    fn unbound_toggle_defaults_enabled_and_unbinding_restores_hold_only_operation() {
        let shared = Shared::with_config(Config {
            hold: 69,
            toggle: 5,
            ..Default::default()
        });
        shared.change(|c| c.enabled = false);
        shared.change(|c| c.config.toggle = 0);
        let mut c = shared.control.lock().unwrap();
        assert!(c.enabled);
        c.require_hold_release = false;
        assert!(eligible(&c, true));
        let fresh = Shared::with_config(Config {
            hold: 69,
            ..Default::default()
        });
        assert!(eligible(&fresh.control.lock().unwrap(), true));
    }
    #[test]
    fn a_shared_engine_can_claim_only_one_capture_worker() {
        let s = Shared::with_config(Config::default());
        assert!(s.claim_worker());
        assert!(!s.claim_worker());
    }
}
