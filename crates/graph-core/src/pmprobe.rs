//! THROWAWAY probe for perf-pm-coherent-sort step 2. Never committed.

use std::sync::atomic::{AtomicU64, Ordering};

pub const NAMES: [&str; 4] = ["fill+count+prefix", "scatter", "Rows::sort", "gravity"];

static NS: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];
static CALLS: [AtomicU64; 4] = [
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
    AtomicU64::new(0),
];

pub struct Span(pub usize, pub std::time::Instant);

impl Span {
    pub fn new(which: usize) -> Span {
        Span(which, std::time::Instant::now())
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        NS[self.0].fetch_add(self.1.elapsed().as_nanos() as u64, Ordering::Relaxed);
        CALLS[self.0].fetch_add(1, Ordering::Relaxed);
    }
}

pub fn rows(ticks: u64) -> Vec<String> {
    (0..4)
        .map(|i| {
            let ns = NS[i].load(Ordering::Relaxed) as f64;
            let calls = CALLS[i].load(Ordering::Relaxed).max(1);
            format!(
                "| {} | {} | {:.3} |",
                NAMES[i],
                calls / ticks.max(1),
                ns / 1e6 / (ticks.max(1) as f64)
            )
        })
        .collect()
}