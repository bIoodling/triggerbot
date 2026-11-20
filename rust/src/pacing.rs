//! Backpressure only BETWEEN completed passes. Never wait between match and click.
use std::{
    ptr::null,
    time::{Duration, Instant},
};
use windows_sys::Win32::{Foundation::*, System::Threading::*};
pub struct Pacer {
    timer: HANDLE,
}
impl Pacer {
    pub fn new() -> Self {
        unsafe {
            let mut timer = CreateWaitableTimerExW(null(), null(), 0x2, TIMER_ALL_ACCESS);
            if timer.is_null() {
                timer = CreateWaitableTimerExW(null(), null(), 0, TIMER_ALL_ACCESS);
            }
            Self { timer }
        }
    }
    pub fn wait_since(&self, started: Instant) {
        let remaining = Duration::from_millis(1).saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return;
        }
        unsafe {
            if !self.timer.is_null() {
                let ticks = ((remaining.as_nanos() + 99) / 100).min(i64::MAX as u128) as i64;
                let due = -ticks.max(1);
                if SetWaitableTimer(self.timer, &due, 0, None, null(), 0) != 0 {
                    let _ = WaitForSingleObject(self.timer, 10);
                    return;
                }
            }
        }
        std::thread::sleep(remaining);
    }
}
impl Drop for Pacer {
    fn drop(&mut self) {
        unsafe {
            if !self.timer.is_null() {
                CancelWaitableTimer(self.timer);
                CloseHandle(self.timer);
            }
        }
    }
}
