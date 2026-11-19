//! One detection event owns at most one dispatch, including failed dispatches.
use std::time::Duration;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Idle,
    Armed,
    Fired,
    WaitingForClear,
}
pub struct Latch {
    pub phase: Phase,
    clear_since: Option<Duration>,
    clear_observations: u32,
}
impl Latch {
    pub fn new() -> Self {
        Self {
            phase: Phase::Idle,
            clear_since: None,
            clear_observations: 0,
        }
    }
    pub fn reset(&mut self, active: bool) {
        self.phase = if active { Phase::Armed } else { Phase::Idle };
        self.invalidate_clear();
    }
    pub fn invalidate_clear(&mut self) {
        self.clear_since = None;
        self.clear_observations = 0;
    }
    // Call only after a fresh GPU result or a successful DXGI timeout that
    // confirms the identical cached ROI is unchanged. Errors are not absence.
    pub fn observe(&mut self, found: bool, now: Duration, count: u32, delay: Duration) -> bool {
        if self.phase == Phase::Idle {
            return false;
        }
        if found {
            self.invalidate_clear();
            if self.phase == Phase::Armed {
                self.phase = Phase::Fired;
                return true;
            }
        } else if self.phase == Phase::WaitingForClear {
            let since = *self.clear_since.get_or_insert(now);
            self.clear_observations = self.clear_observations.saturating_add(1);
            if self.clear_observations >= count.max(2) && now.saturating_sub(since) >= delay {
                self.phase = Phase::Armed;
                self.invalidate_clear();
            }
        }
        false
    }
    pub fn dispatched(&mut self) {
        if self.phase == Phase::Fired {
            self.phase = Phase::WaitingForClear;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample(l: &mut Latch, found: bool, ms: u64) -> bool {
        let fired = l.observe(
            found,
            Duration::from_millis(ms),
            2,
            Duration::from_millis(8),
        );
        l.dispatched();
        fired
    }
    #[test]
    fn continuous_highlight_and_qualified_clear() {
        let mut l = Latch::new();
        l.reset(true);
        assert!(sample(&mut l, true, 0));
        for t in 1..20 {
            assert!(!sample(&mut l, true, t));
        }
        assert!(!sample(&mut l, false, 20));
        // An unchanged-desktop observation after the timeout completes clearing.
        assert!(!sample(&mut l, false, 28));
        assert!(sample(&mut l, true, 29));
        assert!(!sample(&mut l, true, 30));
    }
    #[test]
    fn flicker_errors_and_disarm_cannot_rearm() {
        let mut l = Latch::new();
        l.reset(true);
        assert!(sample(&mut l, true, 0));
        sample(&mut l, false, 1);
        assert!(!sample(&mut l, true, 8));
        sample(&mut l, false, 9);
        l.invalidate_clear();
        sample(&mut l, false, 30);
        assert!(!sample(&mut l, true, 31));
        l.reset(false);
        assert!(!sample(&mut l, true, 32));
        l.reset(true);
        assert!(sample(&mut l, true, 33));
    }
    #[test]
    fn failed_dispatch_also_consumes_appearance() {
        let mut l = Latch::new();
        l.reset(true);
        assert!(l.observe(true, Duration::ZERO, 2, Duration::ZERO));
        assert!(!l.observe(true, Duration::ZERO, 2, Duration::ZERO));
        l.dispatched();
        assert!(!sample(&mut l, true, 5));
    }
}
