//! `graph-cli tick --passes`: where a tick's wall time goes, pass by pass. [`Timed`] wraps
//! [`Threads`] and adds each `run` call's wall time to its kernel's row. What no row covers is
//! the tick's own code between passes, the part no worker count shortens: the counting sorts,
//! the centering fold, gravity.
//!
//! Caveat: a kernel is named by its type, so the call sites of one kernel (the three velocity
//! merges, the two bounds folds) share a row; the calls column says how many ran. A pass's
//! time includes `Threads`' spawn and join, which the wasm pool pays as a wake and a wait
//! instead, so the serial row is the number to carry to the browser, not the pass rows.

use crate::exec_native::Threads;
use graph_core::exec::{Runner, StepRange};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// A [`Threads`] runner that times every pass it runs.
#[derive(Default)]
pub struct Timed {
    rows: RefCell<BTreeMap<&'static str, (u32, Duration)>>,
}

impl Runner for Timed {
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>) {
        let started = Instant::now();
        Threads.run(kernel, workers, out);
        let spent = started.elapsed();
        let mut rows = self.rows.borrow_mut();
        let row = rows.entry(kernel_name::<O>()).or_default();
        row.0 += 1;
        row.1 += spent;
    }
}

impl Timed {
    /// The pass table over `ticks` ticks that took `total` in all, slowest pass first.
    pub fn table(&self, ticks: u32, total: Duration) -> String {
        let per_tick = |d: Duration| d.as_secs_f64() * 1e3 / f64::from(ticks);
        let total_ms = per_tick(total);
        let rows = self.rows.borrow();
        let mut sorted: Vec<_> = rows.iter().collect();
        sorted.sort_by_key(|row| std::cmp::Reverse(row.1.1));
        let mut lines = vec![
            "| pass | calls/tick | ms/tick | share |".to_owned(),
            "|---|---:|---:|---:|".to_owned(),
        ];
        let mut covered = Duration::ZERO;
        for (name, (calls, spent)) in sorted {
            covered += *spent;
            let ms = per_tick(*spent);
            let calls = f64::from(*calls) / f64::from(ticks);
            lines.push(format!(
                "| `{name}` | {calls:.0} | {ms:.2} | {:.1}% |",
                share(ms, total_ms)
            ));
        }
        let serial = per_tick(total.saturating_sub(covered));
        lines.push(format!(
            "| outside any pass (serial) | — | {serial:.2} | {:.1}% |",
            share(serial, total_ms)
        ));
        lines.push(format!("| tick | — | {total_ms:.2} | 100% |"));
        lines.join("\n")
    }
}

fn share(ms: f64, total: f64) -> f64 {
    if total > 0.0 { 100.0 * ms / total } else { 0.0 }
}

/// `module::Kernel`: the last two segments of the type's path, which name the pass.
fn kernel_name<O>() -> &'static str {
    let full = std::any::type_name::<O>();
    let path = full.split('<').next().unwrap_or(full);
    let mut cut = path.rmatch_indices("::").map(|(at, _)| at);
    cut.nth(1).map_or(path, |at| &path[at + 2..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::exec::Serial;
    use std::ops::Range;

    struct Squares;

    impl StepRange for Squares {
        type Out = u32;

        fn len(&self) -> u32 {
            8
        }

        fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
            for (o, i) in out.iter_mut().zip(range) {
                *o = i * i;
            }
        }
    }

    #[test]
    fn timing_a_pass_leaves_its_bytes_alone_and_counts_its_calls() {
        let timed = Timed::default();
        let (mut got, mut want) = (Vec::new(), Vec::new());
        timed.run(&Squares, 3, &mut got);
        timed.run(&Squares, 3, &mut got);
        Serial.run(&Squares, 1, &mut want);
        assert_eq!(got, want);
        let table = timed.table(1, Duration::from_secs(1));
        assert!(table.contains("| `tests::Squares` | 2 |"), "{table}");
        assert!(table.contains("outside any pass"), "{table}");
    }
}
