use crate::{config::Config, input};
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
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
}
pub struct Shared {
    pub control: Mutex<Control>,
    pub wake: Condvar,
    pub running: AtomicBool,
    pub status: AtomicU8,
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
            }),
            wake: Condvar::new(),
            running: AtomicBool::new(true),
            status: AtomicU8::new(0),
        })
    }
    pub fn change(&self, f: impl FnOnce(&mut Control)) {
        let mut c = self.control.lock().unwrap();
        f(&mut c);
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
#[derive(PartialEq)]
enum Phase {
    Idle,
    Armed,
    Waiting,
}
pub fn start(shared: Arc<Shared>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut gpu = None;
        let mut phase = Phase::Idle;
        let mut generation = 0;
        let mut clear: Option<Instant> = None;
        let mut frames = 0u32;
        while shared.running.load(Ordering::Acquire) {
            let (cfg, version, viewport) = {
                let c = shared.control.lock().unwrap();
                if !c.enabled || !c.held || c.suspended || !input::down(c.config.hold) {
                    phase = Phase::Idle;
                    clear = None;
                    frames = 0;
                    shared
                        .status
                        .store(if c.enabled { 0 } else { 2 }, Ordering::Release);
                    let _ = shared
                        .wake
                        .wait_timeout(c, Duration::from_millis(100))
                        .unwrap();
                    continue;
                }
                (c.config.clone(), c.generation, c.viewport)
            };
            if version != generation || phase == Phase::Idle {
                generation = version;
                phase = Phase::Armed;
                clear = None;
                frames = 0;
            }
            let Some((point, monitor)) = crate::capture::point(&cfg, viewport) else {
                clear = None;
                frames = 0;
                std::thread::sleep(Duration::from_millis(20));
                continue;
            };
            if gpu
                .as_ref()
                .map(|g: &crate::gpu::Detector| g.monitor != monitor)
                .unwrap_or(true)
            {
                gpu = unsafe { crate::gpu::Detector::new(monitor).ok() };
                if gpu.is_none() {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            }
            let result = unsafe { gpu.as_mut().unwrap().frame(point, &cfg) };
            let found = match result {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(_) => {
                    gpu = None;
                    clear = None;
                    frames = 0;
                    continue;
                }
            };
            let c = shared.control.lock().unwrap();
            if c.generation != version
                || !c.enabled
                || !c.held
                || c.suspended
                || !input::down(cfg.hold)
            {
                phase = Phase::Idle;
                continue;
            }
            shared.status.store(1, Ordering::Release);
            if found {
                clear = None;
                frames = 0;
                if phase == Phase::Armed {
                    phase = Phase::Waiting;
                    if cfg.output != 0 {
                        input::send(cfg.output);
                    }
                }
            } else if phase == Phase::Waiting {
                let since = clear.get_or_insert_with(Instant::now);
                frames += 1;
                if frames >= cfg.clear_frames
                    && since.elapsed() >= Duration::from_millis(cfg.clear_ms)
                {
                    phase = Phase::Armed;
                    clear = None;
                    frames = 0;
                }
            }
        }
    })
}
