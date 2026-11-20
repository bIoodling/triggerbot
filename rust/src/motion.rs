use std::time::{Duration, Instant};
#[derive(Clone)]
pub struct Tween {
    from: f32,
    to: f32,
    start: Instant,
    duration: Duration,
}
impl Tween {
    pub fn new(value: f32) -> Self {
        Self {
            from: value,
            to: value,
            start: Instant::now(),
            duration: Duration::ZERO,
        }
    }
    pub fn value(&self) -> f32 {
        let t = if self.duration.is_zero() {
            1.
        } else {
            (self.start.elapsed().as_secs_f32() / self.duration.as_secs_f32()).clamp(0., 1.)
        };
        self.from + (self.to - self.from) * (1. - (1. - t).powi(3))
    }
    pub fn set(&mut self, to: f32, ms: u64) {
        if self.to == to {
            return;
        }
        self.from = self.value();
        self.to = to;
        self.start = Instant::now();
        self.duration = Duration::from_millis(ms);
    }
    pub fn reset(&mut self, value: f32) {
        *self = Self::new(value);
    }
    pub fn active(&self) -> bool {
        self.start.elapsed() < self.duration
    }
}
pub fn mix(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0., 1.);
    let mut c = 0;
    for shift in [0, 8, 16, 24] {
        let x = ((a >> shift) & 255) as f32;
        let y = ((b >> shift) & 255) as f32;
        c |= ((x + (y - x) * t).round() as u32) << shift;
    }
    c
}
