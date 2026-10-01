//! The sweep's claims, as tests: which arms it runs, which it refuses, that every arm it
//! times is byte-equal to the serial arm at the same N, that a deliberately split-sum arm
//! is *not* — the control that keeps the equality check from being vacuous — and that the
//! report it writes carries the machine and the raw runs, not a bare number.
//!
//! Split by the house's 300-line cap: this file holds the arms the sweep runs and the
//! equality it checks, [`layout`] the routing by `--layout` and the same equality for
//! every stage the routes name.

mod layout;
mod report;

use super::markdown::loadavg_from;
use super::route::Control;
use super::*;
use crate::bench::Plan;
use graph_core::Stage;
use graph_core::layout::force::{BarnesHut, Split};

fn plan(sizes: Vec<u32>, repeat: u32) -> Plan {
    Plan {
        sizes,
        repeat,
        out: None,
        ..Plan::for_tests()
    }
}

#[test]
fn the_tier_list_is_scalar_or_threads_and_a_tier_that_is_not_built_is_refused() {
    assert_eq!(
        parse_asked("scalar,threads").expect("both are built"),
        vec![Asked::Scalar, Asked::Threads]
    );
    assert_eq!(
        parse_asked("threads").expect("threads is built"),
        vec![Asked::Threads]
    );
    for unbuilt in ["simd", "gpu"] {
        let err = parse_asked(unbuilt).expect_err("not built yet");
        assert!(
            err.contains(unbuilt),
            "the refusal must name the tier: {err:?}"
        );
    }
    assert!(
        parse_asked("scalar,nonsense").is_err(),
        "an unknown word must not fall back to the default list"
    );
}

#[test]
fn threads_expands_over_the_worker_counts_the_flag_names() {
    assert_eq!(
        arms(&[Asked::Threads], &[2, 4, 7]),
        vec![Tier::Threads(2), Tier::Threads(4), Tier::Threads(7)]
    );
    assert_eq!(arms(&[Asked::Scalar], &[2, 4, 7]), vec![Tier::Scalar]);
    assert_eq!(
        arms(&[Asked::Scalar, Asked::Threads], &[4]),
        vec![Tier::Scalar, Tier::Threads(4)]
    );
}

#[test]
fn every_benched_worker_count_is_one_the_hash_gate_runs_and_one_worker_is_not_repeated() {
    for workers in WORKER_COUNTS {
        assert!(
            crate::hashgate::WORKER_COUNTS.contains(&workers),
            "workers={workers} is timed but the gate proves nothing about it"
        );
    }
    assert!(
        !WORKER_COUNTS.contains(&1),
        "one worker is the scalar arm, which the sweep already times"
    );
}

#[test]
fn the_default_worker_list_is_the_list() {
    // clap's `default_value` can only be a string, so the pinned array and the flag's
    // default are two spellings of one thing. This is what holds them together.
    let parsed: Vec<u32> = WORKERS_DEFAULT
        .split(',')
        .map(|word| word.trim().parse().expect("a number"))
        .collect();
    assert_eq!(parsed, WORKER_COUNTS.to_vec());
}

#[test]
fn a_width_the_gate_does_not_run_is_refused_rather_than_timed() {
    let mut plan = plan(vec![40], 1);
    plan.tiers = Some(vec![Asked::Threads]);
    plan.workers = vec![5];
    let err = entry(&plan).expect_err("5 is not a gated width");
    assert!(
        err.contains('5'),
        "the refusal must name the width: {err:?}"
    );
    plan.workers = vec![4];
    assert!(entry(&plan).expect("4 is gated"), "a gated width runs");
}

#[test]
fn every_arm_is_byte_equal_to_the_serial_arm_at_the_same_size() {
    let layouts = vec![BarnesHut::ID];
    let (cells, _) = run(
        &plan(vec![40], 1),
        &layouts,
        &[Tier::Scalar, Tier::Threads(3)],
    )
    .expect("ran");
    assert_eq!(cells.len(), 2);
    for cell in &cells {
        assert!(
            cell.equal,
            "{} disagreed with the serial arm",
            cell.tier.arm_name()
        );
        assert!(
            cell.median_ms() > 0.0,
            "{} timed nothing",
            cell.tier.arm_name()
        );
        assert_eq!(cell.runs_ms.len(), 1, "one repeat means one run");
    }
}

#[test]
fn an_arm_whose_merge_was_split_is_reported_unequal_rather_than_timed() {
    // The control for the equality check above: `split_sum` is the mutation a wrong
    // partition of the outputs takes, and it is applied to the *non-scalar* arms only, so
    // the reference stays the honest one. Without this, `equal` could be `true` forever.
    let layouts = vec![BarnesHut::ID];
    let (cells, _) = run_under(
        &plan(vec![40], 1),
        &layouts,
        &[Tier::Scalar, Tier::Threads(3)],
        Control {
            split: Split::All,
            ..Control::HONEST
        },
    )
    .expect("ran");
    assert!(cells[0].equal, "the serial arm is its own reference");
    assert!(
        !cells[1].equal,
        "a split merge compared equal: the sweep's byte check proves nothing"
    );
}

#[test]
fn the_order_the_tiers_were_asked_in_cannot_decide_whether_a_cell_is_compared() {
    // `--tiers threads,scalar` puts the reference *last*. A cell that reported `true`
    // because no reference had been seen yet would be the one lie the whole column rests
    // on, and the split control below is what tells the two cases apart: honest arms agree
    // whichever side of the reference they are timed on.
    let layouts = vec![BarnesHut::ID];
    let (honest, _) = run(
        &plan(vec![40], 1),
        &layouts,
        &[Tier::Threads(3), Tier::Scalar],
    )
    .expect("ran");
    assert!(honest[0].equal, "an honest arm is the serial arm's bytes");
    assert!(honest[1].equal, "the serial arm is its own reference");
    let (split, _) = run_under(
        &plan(vec![40], 1),
        &layouts,
        &[Tier::Threads(3), Tier::Scalar],
        Control {
            split: Split::All,
            ..Control::HONEST
        },
    )
    .expect("ran");
    assert!(
        !split[0].equal,
        "a split merge compared equal because the reference was timed second: \
         the sweep's byte check proves nothing in this order"
    );
}

#[test]
fn a_speedup_is_against_the_serial_median_and_the_serial_arm_has_none() {
    let reference = vec![10.0, 10.0, 10.0];
    let cell = Cell {
        layout: BarnesHut::ID,
        n: 100,
        tier: Tier::Threads(4),
        runs_ms: vec![2.5, 2.5, 2.5],
        equal: true,
    };
    let serial = Cell {
        layout: BarnesHut::ID,
        n: 100,
        tier: Tier::Scalar,
        runs_ms: reference,
        equal: true,
    };
    assert_eq!(cell.median_ms(), 2.5);
    assert_eq!(cell.speedup(&serial), Some(4.0));
    assert_eq!(serial.speedup(&serial), None);
}

#[test]
fn the_load_average_is_the_first_three_fields_and_an_unreadable_host_says_so() {
    let read = |_: &str| -> std::io::Result<String> { Ok("0.78 0.88 0.72 4/1109 2190861".into()) };
    assert_eq!(loadavg_from(read), "0.78 0.88 0.72");
    let missing = |_: &str| -> std::io::Result<String> { Err(std::io::Error::other("no /proc")) };
    assert_eq!(loadavg_from(missing), "unavailable");
}
