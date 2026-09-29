//! `partition(n, workers)`: where the slices are.
//!
//! The rule is a convention, so it is pinned by **exact vectors** first and by its
//! properties second — the vectors are what a mutant that moves a boundary fails, and the
//! properties are what say the vectors are the rule rather than three coincidences.

use super::super::partition;
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
