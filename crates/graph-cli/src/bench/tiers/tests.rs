//! The sweep's claims, as tests: which arms it runs, which it refuses, that every arm it
//! times is byte-equal to the serial arm at the same N, that a deliberately split-sum arm
//! is *not* — the control that keeps the equality check from being vacuous — and that the
//! report it writes carries the machine and the raw runs, not a bare number.

use super::markdown::{loadavg_from, markdown};
use super::route::ROUTES;
use super::*;
use crate::bench::Plan;
use graph_core::Grid;
use graph_core::Stage;
use graph_core::layout::force::{BarnesHut, Split};
use graph_core::layout::{circular::ring, spiral};

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

fn base_cells() -> Vec<Cell> {
    let sizes = [220_u32, 10_000, 100_000];
    let speeds = [
        (220, [0.76, 0.65, 0.51]),
        (10_000, [1.23, 1.70, 2.85]),
        (100_000, [1.46, 2.28, 3.82]),
    ];
    let mut cells = Vec::new();
    for (&n, rows) in sizes.iter().zip(&speeds) {
        cells.push(Cell {
            layout: BarnesHut::ID,
            n,
            tier: Tier::Scalar,
            runs_ms: vec![100.0],
            equal: true,
        });
        for (w, s) in rows.1.iter().enumerate() {
            cells.push(Cell {
                layout: BarnesHut::ID,
                n,
                tier: Tier::Threads(w as u32 + 2),
                runs_ms: vec![100.0 / s],
                equal: true,
            });
        }
    }
    cells
}

fn with_runs(cells: &[Cell], ms: f64) -> Vec<Cell> {
    cells
        .iter()
        .map(|c| Cell {
            runs_ms: vec![ms],
            ..c.clone()
        })
        .collect()
}

#[test]
fn the_crossover_is_the_bracket_between_the_last_loss_and_the_first_win() {
    let cells = base_cells();
    assert_eq!(
        crossover(&cells),
        Some(((BarnesHut::ID, 220), (BarnesHut::ID, 10_000))),
        "the bracket is what a threshold row is written from"
    );
    assert_eq!(crossover(&with_runs(&cells, 50.0)), None, "all wins");
    assert_eq!(crossover(&with_runs(&cells, 200.0)), None, "all losses");
}

fn host() -> Host {
    Host {
        nproc: 16,
        load_start: "0.78 0.88 0.72".into(),
        load_end: "0.31 0.55 0.60".into(),
    }
}

fn cell(n: u32, tier: Tier, runs: &[f64]) -> Cell {
    Cell {
        layout: BarnesHut::ID,
        n,
        tier,
        runs_ms: runs.to_vec(),
        equal: true,
    }
}

#[test]
fn the_report_carries_the_host_the_repeat_count_every_run_and_the_speedups() {
    let cells = vec![
        cell(220, Tier::Scalar, &[100.0, 100.0, 100.0]),
        cell(220, Tier::Threads(4), &[200.0, 200.0, 200.0]),
        cell(10_000, Tier::Scalar, &[100.0, 100.0, 100.0]),
        cell(10_000, Tier::Threads(4), &[40.0, 40.0, 40.0]),
    ];
    let text = markdown(&plan(vec![220, 10_000], 3), &cells, &host());
    for want in [
        "nproc 16",
        "0.78 0.88 0.72",
        "0.31 0.55 0.60",
        "repeat 3",
        "threads 4",
        "scalar",
        "100.00",
        "40.00",
        "2.50x",
        "0.50x",
        "220",
        "10000",
        "this sweep measured n=220, n=10000",
        "Ponytail",
    ] {
        assert!(text.contains(want), "missing {want:?}");
    }
    // The layout is on every row: a table holding two stages' rows must not read as one
    // ladder, and the subject sentence says what was timed.
    assert!(
        text.contains(BarnesHut::ID),
        "the row does not name its layout"
    );
    assert!(text.contains("the Barnes-Hut stage"), "the subject: {text}");
}

/// The sweep times the layout `--layout` names, and refuses every other one by name.
///
/// Without this, `bench --layout layout.grid --tiers` measured Barnes-Hut and printed the
/// grid's name nowhere, so a speedup column could not be attributed to a stage at all. The
/// default is Barnes-Hut, which is what keeps `docs/measurements/phase11-threads.md`
/// reproducible with no `--layout`.
#[test]
fn the_sweep_times_the_layout_the_flag_names_and_refuses_one_it_cannot() {
    let mut plan = plan(vec![40], 1);
    assert_eq!(layouts(&plan).expect("the default"), vec![BarnesHut::ID]);
    for id in [Grid::ID, ring::ID, spiral::ID] {
        plan.layouts = vec![id.to_string()];
        assert_eq!(layouts(&plan).expect("routed"), vec![id]);
    }
    plan.layouts = vec!["layout.forceatlas2".into()];
    let err = layouts(&plan).expect_err("not a tier route");
    assert!(err.contains("layout.forceatlas2"), "{err}");
    for id in ROUTES {
        assert!(
            err.contains(id),
            "the refusal must name every route it does have, not {id:?}"
        );
    }
}

/// Every routed layout's threaded arm is byte-equal to its own serial arm, and the
/// rescale control turns red exactly the two stages that end in that merge.
#[test]
fn every_routed_layout_is_byte_equal_at_every_width_and_the_merge_control_bites() {
    let layouts = vec![Grid::ID, ring::ID, spiral::ID];
    let arms = [
        Tier::Scalar,
        Tier::Threads(1),
        Tier::Threads(3),
        Tier::Threads(7),
    ];
    let (honest, _) = run(&plan(vec![40], 1), &layouts, &arms).expect("ran");
    for cell in &honest {
        assert!(
            cell.equal,
            "{} {} at {:?} disagreed with its own serial arm",
            cell.layout, cell.n, cell.tier
        );
    }
    // And the negative control for the `equal` column: the two stages with a merge go red,
    // and the grid — which has none — does not. A control that turned all three red would
    // be reaching the gather, not the merge.
    let (split, _) = run_under(
        &plan(vec![40], 1),
        &layouts,
        &arms,
        Control {
            split_rescale: true,
            ..Control::HONEST
        },
    )
    .expect("ran");
    for cell in &split {
        if cell.tier == Tier::Scalar {
            // The reference is always the honest one — a control that reached it would
            // agree with the arms it is meant to contradict.
            assert!(cell.equal, "{}: the reference was mutated", cell.layout);
            continue;
        }
        let want = cell.layout != Grid::ID;
        assert_eq!(
            !cell.equal,
            want,
            "{} at {:?}: a rescale control that moved {} is reaching the wrong half",
            cell.layout,
            cell.tier,
            if want { "the gather" } else { "the grid" }
        );
    }
}
