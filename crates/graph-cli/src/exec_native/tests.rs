//! The threaded executor's claim, as tests: **the worker count changes the schedule and
//! not one byte of the output.** The odd counts are the point — an even split hides a
//! boundary bug behind symmetry.

use super::{Threads, plan};
use graph_core::exec::{Runner, Serial, StepRange};
use std::ops::Range;

/// A gather kernel in the D10 shape, over an exact `f64` sum so a reordering is a
/// visible difference rather than a rounding coincidence: element `i` sums the terms
/// whose index is `k mod n == i`, in ascending `k`.
struct Terms {
    n: u32,
    terms: Vec<f64>,
}

impl Terms {
    fn of(n: u32, count: usize) -> Terms {
        Terms {
            n,
            // Not a round number of terms per output: the ranges cut the terms unevenly.
            terms: (0..count).map(|k| 1.0 / (1 + k % 17) as f64).collect(),
        }
    }
}

impl StepRange for Terms {
    type Out = f32;

    fn len(&self) -> u32 {
        self.n
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f32]) {
        let base = range.start;
        for i in range {
            let mut acc = 0.0_f64;
            for (k, term) in self.terms.iter().enumerate() {
                if k as u32 % self.n == i {
                    acc += *term;
                }
            }
            out[(i - base) as usize] = acc as f32;
        }
    }
}

/// Runs `kernel` on `workers` threads.
fn run_threads_under(kernel: &Terms, workers: u32) -> Vec<f32> {
    let mut out = Vec::new();
    Threads.run(kernel, workers, &mut out);
    out
}

#[test]
fn every_worker_count_gives_the_serial_bytes() {
    for (n, count) in [(1_u32, 1_usize), (7, 11), (8, 29), (220, 733), (1000, 1499)] {
        let kernel = Terms::of(n, count);
        let mut scalar = Vec::new();
        Serial.run(&kernel, 1, &mut scalar);
        for workers in [2_u32, 3, 4, 7, 8] {
            assert_eq!(
                run_threads_under(&kernel, workers),
                scalar,
                "n={n} on {workers} workers diverged from serial"
            );
        }
    }
}

/// A host reporting **zero** workers gets the serial bytes, not a column of zeros: no
/// threads is a single-threaded layout, and a runner that forwarded `0` to `partition`
/// would hand back a wrong answer instead of a slow one.
#[test]
fn zero_workers_is_the_serial_path_and_never_a_column_of_zeros() {
    let kernel = Terms::of(8, 29);
    let mut scalar = Vec::new();
    Serial.run(&kernel, 1, &mut scalar);
    let mut none = Vec::new();
    Threads.run(&kernel, 0, &mut none);
    assert_eq!(none, scalar);
    assert!(
        none.iter().any(|v| *v != 0.0),
        "the column must be the real one"
    );
}

#[test]
fn the_worker_counts_the_gate_uses_all_produce_a_legal_plan() {
    for n in [0_u32, 1, 7, 220, 1000] {
        for workers in [1_u32, 2, 3, 4, 7] {
            let ranges = plan(n, workers);
            assert_eq!(ranges.len() as u32, n.min(workers), "n={n} w={workers}");
            let mut at = 0;
            for range in &ranges {
                assert!(!range.is_empty() && range.start == at, "{range:?} at {at}");
                at = range.end;
            }
            assert_eq!(at, n);
        }
    }
}

#[test]
fn no_workers_or_no_work_is_an_empty_column_not_a_hang() {
    let kernel = Terms::of(0, 0);
    assert!(run_threads_under(&kernel, 4).is_empty());
    let kernel = Terms::of(5, 5);
    assert_eq!(run_threads_under(&kernel, 0), run_threads_under(&kernel, 1));
}

#[test]
fn more_workers_than_outputs_still_writes_every_output_once() {
    let kernel = Terms::of(3, 7);
    assert_eq!(
        run_threads_under(&kernel, 16),
        run_threads_under(&kernel, 1),
        "16 workers over 3 outputs must not drop or duplicate one"
    );
}

#[test]
fn the_callers_buffer_is_overwritten_rather_than_appended_to() {
    let kernel = Terms::of(6, 13);
    let mut out = vec![7.0_f32; 6];
    Threads.run(&kernel, 4, &mut out);
    assert_eq!(out.len(), 6);
    assert_eq!(out, run_threads_under(&kernel, 1));
    Threads.run(&Terms::of(0, 0), 4, &mut out);
    assert!(out.is_empty());
}

/// The negative control for this module's claim: a kernel whose ranges cut one
/// element's *sum* rather than its outputs must not compare equal, or the equality
/// tests above would pass for a reason that has nothing to do with correctness.
#[test]
fn a_kernel_that_splits_a_sum_across_threads_is_caught() {
    /// Sums over the terms **inside** the range, so a range boundary cuts one output's
    /// sum in two — precisely the mutation `GM_MUTATE_SPLIT_SUM` stands for.
    struct Splitting<'a>(&'a Terms);

    impl StepRange for Splitting<'_> {
        type Out = f32;

        fn len(&self) -> u32 {
            self.0.n
        }

        fn step_range(&self, range: Range<u32>, out: &mut [f32]) {
            for slot in out.iter_mut() {
                *slot = 0.0;
            }
            let base = range.start;
            for (k, term) in self
                .0
                .terms
                .iter()
                .enumerate()
                .take(range.end as usize)
                .skip(range.start as usize)
            {
                let i = k as u32 % self.0.n;
                if range.contains(&i) {
                    out[(i - base) as usize] += *term as f32;
                }
            }
        }
    }

    let kernel = Terms::of(8, 29);
    let n = kernel.len();
    let mut scalar = Vec::new();
    Serial.run(&kernel, 1, &mut scalar);
    // Each range its own sub-column, laid end to end — the same assembly `run` does, so
    // the only difference from the honest run is which terms each range sees.
    let mut split = Vec::new();
    for range in plan(n, 3) {
        let mut span = vec![0.0; range.len()];
        Splitting(&kernel).step_range(range, &mut span);
        split.extend_from_slice(&span);
    }
    assert_ne!(
        split, scalar,
        "a split sum compared equal: the equality tests prove nothing"
    );
}
