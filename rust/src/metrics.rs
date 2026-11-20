use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
pub struct Timing {
    count: AtomicU64,
    total: AtomicU64,
    min: AtomicU64,
    max: AtomicU64,
    last: AtomicU64,
}
impl Timing {
    pub fn new() -> Self {
        Self {
            count: AtomicU64::new(0),
            total: AtomicU64::new(0),
            min: AtomicU64::new(u64::MAX),
            max: AtomicU64::new(0),
            last: AtomicU64::new(0),
        }
    }
    pub fn record(&self, time: Duration) {
        let ns = time.as_nanos().min(u64::MAX as u128) as u64;
        self.count.fetch_add(1, Ordering::Relaxed);
        self.total.fetch_add(ns, Ordering::Relaxed);
        self.min.fetch_min(ns, Ordering::Relaxed);
        self.max.fetch_max(ns, Ordering::Relaxed);
        self.last.store(ns, Ordering::Relaxed);
    }
    pub fn report(&self) -> serde_json::Value {
        let count = self.count.load(Ordering::Relaxed);
        if count == 0 {
            return serde_json::json!({"samples":0,"last_us":null,"min_us":null,"max_us":null,"mean_us":null});
        }
        serde_json::json!({"samples":count,"last_us":self.last.load(Ordering::Relaxed) as f64/1000.,"min_us":self.min.load(Ordering::Relaxed) as f64/1000.,"max_us":self.max.load(Ordering::Relaxed) as f64/1000.,"mean_us":self.total.load(Ordering::Relaxed) as f64/count as f64/1000.})
    }
}
