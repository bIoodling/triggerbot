pub mod calibration;
pub mod capture;
pub mod config;
pub mod event;
pub mod input;
pub mod install;
pub mod motion;
pub mod render;
pub mod screen;
pub mod tray;
pub mod trigger;
pub mod ui;
#[cfg(test)]
mod verification;
pub mod windowing;
pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

pub mod metrics;
pub mod pacing;
