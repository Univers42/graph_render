//! The flag's own vocabulary: which layouts a plan resolves to, what a ceiling means, and
//! the difference between a **refusal** (exit 0, reported) and a **failure** (exit 1).

use super::super::campaign::{SETTLE_TICKS, largest_fitting, median, settle_ms};
use super::super::*;
use graph_core::StageError;

/// A plan with only the two flags these tests vary; every other field at its default.
pub(super) fn plan(past_ceiling: bool, dry_run: bool) -> Plan {
    Plan {
        past_ceiling,
        dry_run,
        ..Plan::for_tests()
    }
}

fn path(n: u32) -> Topology {
    let (nodes, edges) = seeded_model(0, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("fits")
}

fn refuses(_: &Topology) -> Result<Geometry, StageError> {
    Err(StageError::Param {
        name: "test",
        rule: "always refused",
    })
}

/// A registered entry whose run always fails, at a chosen ceiling.
fn failing(ceiling: u64) -> Capability {
    let mut entry = *registry::find("layout.grid").expect("registered");
    entry.run = refuses;
    entry.meta.scale_ceiling = ceiling;
    entry
}

#[test]
fn the_default_set_is_the_four_phase6_layouts_and_an_unknown_id_is_refused() {
    let ids: Vec<&str> = resolve(&[])
        .expect("registered")
        .iter()
        .map(|e| e.id)
        .collect();
    assert_eq!(ids, PHASE6);
    let one = resolve(&["layout.grid".into()]).expect("registered");
    assert_eq!(one.len(), 1);
    let err = resolve(&["layout.grid".into(), "layout.none".into()]).map(drop);
    assert_eq!(err, Err("layout.none: not registered".into()));
}

#[test]
fn a_failure_inside_the_ceiling_fails_and_past_it_is_only_reported() {
    let topology = path(12);
    assert_eq!(row(&plan(false, false), &failing(12), &topology), Ok(false));
    assert_eq!(row(&plan(true, false), &failing(11), &topology), Ok(true));
}

#[test]
fn past_the_ceiling_is_refused_and_a_dry_run_runs_nothing() {
    let topology = path(12);
    // Refused and dry runs never reach `run`, so the failing entry passes.
    assert_eq!(row(&plan(false, false), &failing(11), &topology), Ok(true));
    assert_eq!(row(&plan(false, true), &failing(12), &topology), Ok(true));
    assert_eq!(row(&plan(true, true), &failing(11), &topology), Ok(true));
    let grid = registry::find("layout.grid").expect("registered");
    assert_eq!(row(&plan(false, false), grid, &topology), Ok(true));
}

#[test]
fn stress_is_zero_on_an_isometric_path_and_pinned_on_a_folded_one() {
    let (source, target) = ([0, 1], [1, 2]);
    assert_eq!(stress(&[0.0, 2.0, 4.0], &[0.0; 3], &source, &target), 0.0);
    // Folded: node 2 back on node 0. de = ee = 4, dd = 12, so sqrt(8 / 12).
    let folded = stress(&[0.0, 1.0, 0.0], &[0.0; 3], &source, &target);
    assert_eq!(folded, (8.0_f64 / 12.0).sqrt());
}

#[test]
fn a_collapsed_layout_or_an_edgeless_graph_scores_the_worst_stress() {
    assert_eq!(stress(&[5.0; 3], &[5.0; 3], &[0, 1], &[1, 2]), 1.0);
    assert_eq!(stress(&[0.0, 1.0], &[0.0, 0.0], &[], &[]), 1.0);
}

#[test]
fn unreachable_pairs_are_skipped_so_each_component_is_scored_alone() {
    // Two isometric edges far apart: every reachable pair is exact, so 0.
    let (x, y) = ([0.0, 1.0, 100.0, 101.0], [0.0; 4]);
    assert_eq!(stress(&x, &y, &[0, 2], &[1, 3]), 0.0);
}

#[test]
fn the_median_of_an_odd_count_is_the_middle_value_and_of_an_even_one_the_mean() {
    assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
    assert_eq!(median(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    assert_eq!(median(vec![]), 0.0);
    assert_eq!(median(vec![7.0]), 7.0);
}

#[test]
fn settle_is_the_tick_times_the_112_ticks_alpha_decay_needs() {
    assert_eq!(SETTLE_TICKS, 112);
    assert_eq!(settle_ms(0.25), 28.0);
}

#[test]
fn the_crossover_is_the_largest_n_that_fits_the_budget_and_none_fits_when_all_over() {
    let samples = [(220, 1.0), (10_000, 8.0), (100_000, 40.0)];
    assert_eq!(largest_fitting(&samples, 16.67), Some(10_000));
    // Exactly at the budget still fits: a crossover is not a regression test.
    assert_eq!(largest_fitting(&samples, 8.0), Some(10_000));
    assert_eq!(largest_fitting(&samples, 0.5), None);
    assert_eq!(largest_fitting(&[], 16.67), None);
}

/// The graph both arms lay out: the TypeScript oracle's own `buildSyntheticModel`, which
/// graph-core's synthetic model is a call-for-call port of. Pinned here in the native arm
/// and in the harness, so a drift in either is a number, not a silent difference in what
/// the two arms were measured on.
#[test]
fn the_model_the_oracle_arm_lays_out_is_the_model_this_arm_lays_out() {
    let (nodes, edges) = seeded_model(0, 6, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    assert_eq!(
        (nodes.len(), edges.len()),
        (6, 9),
        "the pinned six-node model changed on this side of the port"
    );
    assert_eq!(topology.edge_count(), 9);
}

/// The campaign makes the same refusal a `bench` row makes. It used to label and run every
/// size, and the gate row ran spectral and ForceAtlas2 at 10⁶ until it was killed.
#[test]
fn the_campaign_refuses_past_the_ceiling_and_names_what_it_refused() {
    use super::super::campaign::{refused, report, run};
    let asked = Plan {
        sizes: vec![12, 701],
        layouts: vec!["layout.spectral".into()],
        ..Plan::for_tests()
    };
    let rows = run(&asked).expect("runs");
    let sizes: Vec<u32> = rows[0].1.iter().map(|s| s.n).collect();
    assert_eq!(sizes, [12]);
    let text = report::markdown(&asked, &rows, 16.67);
    assert!(
        text.contains("refused n=[701]: past its scale_ceiling of 700"),
        "{text}"
    );
    let spectral = registry::find("layout.spectral").expect("registered");
    assert!(!refused(&plan(true, false), spectral, 701));
}
