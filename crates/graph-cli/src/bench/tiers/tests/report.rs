//! What the report says: the crossover bracket read off the table, and the table's own
//! columns — the layout on every row, the host, the raw runs and the speedups.
//!
//! Split from the parent module for the house's 300-line cap. `plan` comes from there
//! rather than being restated, so a fixture the parent changes is a fixture this file sees.

use super::super::markdown::markdown;
use super::super::{Cell, Host, Tier, crossover};
use super::plan;
use graph_core::Stage as _;
use graph_core::layout::force::BarnesHut;

pub(super) fn base_cells() -> Vec<Cell> {
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

pub(super) fn host() -> Host {
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
