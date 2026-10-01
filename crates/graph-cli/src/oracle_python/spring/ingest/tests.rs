//! The measuring half's own tests: the deficit, the two exact assertions, and every way a
//! case can be refused rather than scored.
//!
//! The refusals are the half that matters and they are all here, because a differential
//! that scores a malformed case is worse than one that cannot run: it reports a number.
//! Each test therefore builds a *wrong* case on purpose — an arm off the origin, an arm
//! that skipped the rescale, two arms that disagree about which case this is, edge columns
//! that do not cover every node, a seed past the manifest — and holds that each is refused.

use super::measure::{centred_extent, check_manifest, deficit, graph, percentile};
use serde_json::{Value, json};

/// A 5-node path's two arms, at `scale = 0.5`: `straight` spreads the nodes evenly by
/// hop distance and `crowded` piles the middle three on one point, so both satisfy the
/// rescale contract while only the first respects the graph's distances. Both are
/// centred with extent exactly 0.5, which is what the contract checks.
fn arm(x: &[f64]) -> Value {
    json!({
        "seed": 0, "n": 5, "source": [0, 1, 2, 3], "target": [1, 2, 3, 4],
        "params": { "iterations": 50, "threshold": 1e-4, "scale": 0.5 },
        "spring": { "x": x, "y": vec![0.0; x.len()] },
    })
}

fn straight() -> Value {
    arm(&[0.0, 0.25, 0.5, 0.75, 1.0])
}

fn crowded() -> Value {
    arm(&[0.0, 0.5, 0.5, 0.5, 1.0])
}

#[test]
fn a_drawing_that_respects_the_distances_is_a_zero_deficit() {
    let got = deficit(&straight(), &crowded()).expect("scores");
    assert_eq!(
        got,
        Some(0.0),
        "we are not worse, so the deficit is exactly zero"
    );
}

#[test]
fn a_drawing_that_crowds_the_middle_is_a_positive_deficit() {
    let got = deficit(&crowded(), &straight()).expect("scores");
    let deficit = got.expect("a 5-node path has pairs to correlate");
    assert!(
        (0.2..0.4).contains(&deficit),
        "the deficit is the correlation gap, got {deficit}"
    );
}

#[test]
fn a_one_node_graph_is_the_origin_on_both_arms() {
    let lone = json!({
        "seed": 3, "n": 1, "source": [], "target": [],
        "params": { "iterations": 50, "threshold": 1e-4, "scale": 5.0 },
        "spring": { "x": [0.0], "y": [0.0] },
    });
    assert_eq!(
        deficit(&lone, &lone).expect("scores"),
        None,
        "no pair to correlate"
    );
    let off = json!({
        "seed": 3, "n": 1, "source": [], "target": [],
        "params": { "iterations": 50, "threshold": 1e-4, "scale": 5.0 },
        "spring": { "x": [1.0], "y": [0.0] },
    });
    assert!(
        deficit(&off, &lone).is_err(),
        "the origin is not negotiable"
    );
}

#[test]
fn a_drawing_that_skipped_the_rescale_is_refused_rather_than_scored() {
    let mut ours = straight();
    ours["spring"]["x"] = json!([0.0, 0.5, 1.0, 1.5, 2.0]);
    let err = deficit(&ours, &straight()).expect_err("extent 1.0, not 0.5");
    assert!(err.contains("spans"), "{err}");
}

#[test]
fn arms_that_disagree_about_the_case_are_refused() {
    let mut theirs = straight();
    theirs["seed"] = json!(9);
    assert!(deficit(&straight(), &theirs).is_err());
    let mut short = straight();
    short["n"] = json!(3);
    assert!(
        deficit(&straight(), &short).is_err(),
        "a 3-node graph for a 5-node case"
    );
}

#[test]
fn edge_columns_that_do_not_cover_every_node_are_refused() {
    let mut ours = straight();
    ours["source"] = json!([0, 1]);
    ours["target"] = json!([1, 2]);
    let err = graph(&ours, 5).expect_err("describes 3 nodes");
    assert!(err.contains("3 nodes, not 5"), "{err}");
}

#[test]
fn a_case_past_the_manifests_seed_count_is_refused() {
    let manifest = json!({ "seeds": 3 });
    assert!(check_manifest(&manifest, &straight()).is_ok());
    let mut late = straight();
    late["seed"] = json!(3);
    assert!(check_manifest(&manifest, &late).is_err());
}

#[test]
fn the_quantiles_are_nearest_rank_and_never_interpolated() {
    let sorted = [0.0, 1.0, 2.0, 3.0, 4.0];
    assert_eq!(percentile(&sorted, 0.5), 2.0, "ceil(2.5) = 3rd, 1-based");
    assert_eq!(percentile(&sorted, 0.9), 4.0, "ceil(4.5) = 5th, the last");
    assert_eq!(
        percentile(&[], 0.5),
        0.0,
        "no measurement is not a measurement"
    );
    assert_eq!(percentile(&[7.0], 0.9), 7.0);
}

#[test]
fn the_extent_is_measured_about_the_centroid_on_both_axes() {
    let square = [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
    assert_eq!(centred_extent(&square), 1.0);
    assert_eq!(centred_extent(&[(0.0, 0.0), (1.0, 0.0)]), 0.5);
}
