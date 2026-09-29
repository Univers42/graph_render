//! [`partition`]'s rule, the [`StepRange`] contract, and the [`Serial`] runner — plus the
//! negative control that says the equality check is worth running.

use super::{Runner, Serial, StepRange, partition};
use std::ops::Range;

/// The rule restated over every `(n, workers)` the gate and the SDK can ask for:
/// contiguous, ascending, gapless, non-empty, and exactly `min(n, workers)` of them.
///
/// The one exception is `workers == 0`, where the rule *is* "no slices": there is no
/// worker to hand a range to, so nothing covers the outputs and that is the answer, not
/// a gap in the answer.
fn contract(n: u32, workers: u32) {
    let ranges = partition(n, workers);
    if workers == 0 {
        assert!(ranges.is_empty(), "{n} outputs over no workers: no slices");
        return;
    }
    assert_eq!(
        ranges.len() as u32,
        n.min(workers),
        "{n} outputs over {workers} workers: wrong slice count"
    );
    let mut at = 0;
    for range in &ranges {
        assert!(
            !range.is_empty(),
            "{n}/{workers}: an empty slice at {range:?}"
        );
        assert_eq!(
            range.start, at,
            "{n}/{workers}: a gap or overlap at {range:?}"
        );
        at = range.end;
    }
    assert_eq!(at, n, "{n}/{workers}: the slices do not cover the outputs");
}

#[test]
fn the_three_remainder_slices_are_pinned() {
    // 10 over 3: base 3, remainder 1 — the first slice takes the extra element.
    assert_eq!(partition(10, 3), vec![0..4, 4..7, 7..10]);
    // 10 over 4: base 2, remainder 2 — two long slices up front, then two short.
    assert_eq!(partition(10, 4), vec![0..3, 3..6, 6..8, 8..10]);
    // 11 over 4: base 2, remainder 3.
    assert_eq!(partition(11, 4), vec![0..3, 3..6, 6..9, 9..11]);
}

#[test]
fn an_even_division_gives_equal_slices() {
    assert_eq!(partition(8, 4), vec![0..2, 2..4, 4..6, 6..8]);
    assert_eq!(partition(8, 2), vec![0..4, 4..8]);
}

#[test]
fn more_workers_than_outputs_gives_one_slice_each_and_no_empty_ones() {
    assert_eq!(partition(3, 7), vec![0..1, 1..2, 2..3]);
    assert_eq!(partition(1, 4), vec![0..1]);
}

#[test]
fn no_work_or_no_workers_is_no_slice() {
    assert_eq!(partition(0, 4), Vec::<Range<u32>>::new());
    assert_eq!(partition(5, 0), Vec::<Range<u32>>::new());
    assert_eq!(partition(0, 0), Vec::<Range<u32>>::new());
}

#[test]
fn one_worker_takes_everything() {
    assert_eq!(partition(9, 1), vec![0..9]);
}

#[test]
fn the_rule_holds_across_the_gate_sizes_and_worker_counts() {
    for n in [0_u32, 1, 2, 3, 7, 10, 220, 1000, 10_000] {
        for workers in [0_u32, 1, 2, 3, 4, 7] {
            contract(n, workers);
        }
    }
}

#[test]
fn the_slices_depend_on_nothing_but_the_two_arguments() {
    // No host, no clock, no allocation identity: the same call twice is the same call.
    assert_eq!(partition(220, 7), partition(220, 7));
    // A different worker count is the only thing that moves a boundary.
    assert_ne!(partition(220, 7), partition(220, 4));
}

/// A gather-form kernel of the shape the contract names: element `i` reads the state
/// column and accumulates *its own* terms in index order, writing only `out[i]`. This
/// is the D10 shape the force and bundling kernels were written in.
struct Column {
    values: Vec<f32>,
}

impl StepRange for Column {
    type Out = f32;

    fn len(&self) -> u32 {
        self.values.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f32]) {
        let base = range.start;
        for i in range {
            let mut acc = 0.0_f32;
            for k in 0..self.values.len() {
                if k % 3 == (i as usize) % 3 {
                    acc += self.values[k];
                }
            }
            out[(i - base) as usize] = acc;
        }
    }
}

/// The contract's own claim, as a test: running the ranges in any number of pieces is
/// byte-identical to running the whole thing at once, odd counts included.
#[test]
fn any_partition_of_a_gather_kernel_equals_the_whole_run_bit_for_bit() {
    let column = Column {
        values: (0..37).map(|k| k as f32 * 0.5 - 3.25).collect(),
    };
    let n = column.len();
    let mut whole = vec![0.0; n as usize];
    column.step_range(0..n, &mut whole);
    for workers in [1_u32, 2, 3, 4, 7] {
        // Each range its own sub-column, laid end to end: the assembly any executor
        // does, and the only place the ranges could meet.
        let mut out = Vec::new();
        for range in partition(n, workers) {
            let mut span = vec![0.0; range.len()];
            column.step_range(range, &mut span);
            out.extend_from_slice(&span);
        }
        assert_eq!(out, whole, "{workers} workers changed a byte");
    }
}

/// A kernel with `n` outputs and `t` terms, each term belonging to exactly one output
/// (`k mod n`), accumulated in ascending term order — the same shape as the real force
/// and bundling kernels, one output's sum over its own terms.
struct Gather {
    n: u32,
    terms: Vec<f32>,
}

impl StepRange for Gather {
    type Out = f32;

    fn len(&self) -> u32 {
        self.n
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f32]) {
        let base = range.start;
        for i in range {
            let mut acc = 0.0_f32;
            for (k, term) in self.terms.iter().enumerate() {
                if k as u32 % self.n == i {
                    acc += *term;
                }
            }
            out[(i - base) as usize] = acc;
        }
    }
}

/// The negative control of the equality test above: the same kernel with the partition
/// applied to the **terms** instead of the outputs, which is what "one node's force sum
/// split across two threads" looks like from the inside. Every output is still written
/// exactly once and no two ranges overlap, so the shape of the plan looks right — only
/// the sums are wrong, and float addition does not forgive (D3).
struct TermsNotOutputs<'a>(&'a Gather);

impl StepRange for TermsNotOutputs<'_> {
    type Out = f32;

    fn len(&self) -> u32 {
        self.0.n
    }

    fn step_range(&self, range: Range<u32>, out: &mut [f32]) {
        for slot in out.iter_mut() {
            *slot = 0.0;
        }
        // The terms inside this range only, so an output's sum is cut wherever a range
        // boundary falls inside its terms — the mutation, kept in range-local indexing
        // so it is a wrong *answer* rather than a panic.
        for (k, term) in self.0.terms.iter().enumerate().take(range.end as usize) {
            if k < range.start as usize {
                continue;
            }
            let i = k as u32 % self.0.n;
            if !range.contains(&i) {
                continue;
            }
            out[(i - range.start) as usize] += *term;
        }
    }
}

#[test]
fn the_equality_check_catches_a_sum_split_across_two_ranges() {
    let kernel = Gather {
        // 29 terms over 8 outputs: the terms do not divide into the ranges evenly, so
        // some outputs' sums are cut and others are not.
        n: 8,
        terms: (0..29).map(|k| k as f32 * 0.25 - 1.5).collect(),
    };
    let n = kernel.len();
    let mut whole = vec![0.0; n as usize];
    kernel.step_range(0..n, &mut whole);
    let mut split = Vec::new();
    for range in partition(n, 3) {
        let mut span = vec![0.0; range.len()];
        TermsNotOutputs(&kernel).step_range(range, &mut span);
        split.extend_from_slice(&span);
    }
    assert_ne!(
        split, whole,
        "a sum split across two threads must not compare equal, or the check proves nothing"
    );
    // And the honest kernel over the very same 3-way partition is equal, so the
    // difference above is the split and not the partitioning.
    let mut honest = Vec::new();
    for range in partition(n, 3) {
        let mut span = vec![0.0; range.len()];
        kernel.step_range(range, &mut span);
        honest.extend_from_slice(&span);
    }
    assert_eq!(honest, whole);
}

/// The reference runner's claim: whatever worker count the host reports, the bytes are
/// the whole-run bytes. `Serial` is the tier 1a path *and* the yardstick for every other
/// runner, so this is the test a threaded executor is measured against.
#[test]
fn the_serial_runner_gives_the_whole_run_bytes_at_every_worker_count() {
    let column = Column {
        values: (0..37).map(|k| k as f32 * 0.5 - 3.25).collect(),
    };
    let n = column.len() as usize;
    let mut reference = Vec::new();
    Serial.run(&column, 1, &mut reference);
    assert_eq!(reference.len(), n);
    for workers in [0_u32, 1, 2, 3, 4, 7, 16] {
        let mut out = vec![7.0; n];
        Serial.run(&column, workers, &mut out);
        assert_eq!(out, reference, "workers={workers}");
    }
}

/// `run` clears and overwrites: a caller's buffer is never appended to, and never keeps a
/// value the kernel did not write. Both halves matter — an executor that forgot to clear
/// would double every column.
#[test]
fn the_runner_clears_the_callers_buffer_rather_than_appending_to_it() {
    let column = Column {
        values: vec![1.0, 2.0, 3.0],
    };
    let mut out = vec![99.0; 3];
    Serial.run(&column, 2, &mut out);
    assert_eq!(out.len(), 3);
    assert_ne!(out, vec![99.0; 3]);
    // A kernel of no outputs empties the buffer, whatever was in it.
    let none = Column { values: Vec::new() };
    Serial.run(&none, 4, &mut out);
    assert!(out.is_empty());
}

/// The contract is generic over the output type, not fixed to `f32`: the force simulation
/// is `f64` and writes two columns at once. This is that case, and it is here because a
/// second trait for `f64` would be the same rule written twice.
#[test]
fn a_pair_of_f64_columns_is_one_kernel_under_the_same_contract() {
    /// Element `i` writes `(x[i]·2, y[i]·2)` — two columns, one output each.
    struct Pairs {
        x: Vec<f64>,
        y: Vec<f64>,
    }

    impl StepRange for Pairs {
        type Out = (f64, f64);

        fn len(&self) -> u32 {
            self.x.len() as u32
        }

        fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
            let base = range.start;
            for i in range {
                out[(i - base) as usize] = (self.x[i as usize] * 2.0, self.y[i as usize] * 2.0);
            }
        }
    }

    let kernel = Pairs {
        x: vec![1.0, 2.0, 3.0, 4.0, 5.0],
        y: vec![-1.0, -2.0, -3.0, -4.0, -5.0],
    };
    let mut whole = Vec::new();
    Serial.run(&kernel, 1, &mut whole);
    assert_eq!(
        whole,
        vec![
            (2.0, -2.0),
            (4.0, -4.0),
            (6.0, -6.0),
            (8.0, -8.0),
            (10.0, -10.0)
        ]
    );
    for workers in [2_u32, 3, 4, 7] {
        let mut out = Vec::new();
        Serial.run(&kernel, workers, &mut out);
        assert_eq!(out, whole, "workers={workers}");
    }
}
