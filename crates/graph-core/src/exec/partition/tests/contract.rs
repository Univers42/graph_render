//! The [`StepRange`] contract, as a kernel and as a control: any division of a gather's
//! outputs is the same computation, and a division of its *terms* is not.

use super::super::{Runner, Serial, StepRange, partition};
use std::ops::Range;

/// A gather-form kernel of the shape the contract names: element `i` reads the state
/// column and accumulates *its own* terms in index order, writing only `out[i]`. This
/// is the D10 shape the force and bundling kernels were written in.
pub struct Column {
    pub(super) values: Vec<f32>,
}

impl Column {
    /// The 37-value column every test here shares, chosen so the sums are not exact in
    /// binary (`k * 0.5 - 3.25`), which is what makes a reordered sum visible at all.
    pub fn fixture() -> Column {
        Column {
            values: (0..37).map(|k| k as f32 * 0.5 - 3.25).collect(),
        }
    }
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

/// The negative control of the equality test: the same kernel with the partition applied
/// to the **terms** instead of the outputs, which is what "one node's force sum split
/// across two threads" looks like from the inside. Every output is still written exactly
/// once and no two ranges overlap, so the shape of the plan looks right — only the sums
/// are wrong, and float addition does not forgive (D3).
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

/// The contract's own claim, as a test: running the ranges in any number of pieces is
/// byte-identical to running the whole thing at once, odd counts included.
#[test]
fn any_partition_of_a_gather_kernel_equals_the_whole_run_bit_for_bit() {
    let column = Column::fixture();
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

/// The contract is generic over the output type, not fixed to `f32`: the force simulation
/// is `f64` and writes two columns at once. This is that case, and it is here because a
/// second trait for `f64` would be the same rule written twice.
#[test]
fn a_pair_of_f64_columns_is_one_kernel_under_the_same_contract() {
    let kernel = make_pairs_kernel();
    let whole = run_whole(&kernel);
    assert_eq!(whole, expected_pairs_output());
    for workers in [2_u32, 3, 4, 7] {
        let out = run_with_workers(&kernel, workers);
        assert_eq!(out, whole, "workers={workers}");
    }
}

fn make_pairs_kernel() -> Pairs {
    Pairs {
        x: vec![1.0, 2.0, 3.0, 4.0, 5.0],
        y: vec![-1.0, -2.0, -3.0, -4.0, -5.0],
    }
}

fn expected_pairs_output() -> Vec<(f64, f64)> {
    vec![
        (2.0, -2.0),
        (4.0, -4.0),
        (6.0, -6.0),
        (8.0, -8.0),
        (10.0, -10.0),
    ]
}

fn run_whole(kernel: &Pairs) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    Serial.run(kernel, 1, &mut out);
    out
}

fn run_with_workers(kernel: &Pairs, workers: u32) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    Serial.run(kernel, workers, &mut out);
    out
}

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
