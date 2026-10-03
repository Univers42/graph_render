use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::notes::NoteCode;

#[test]
fn run_builds_point_nodes_and_polyline_edges_with_the_expected_notes() {
    // A 3-cycle (one edge reversed) plus a 3-layer skip edge (one dummy, well
    // within the production budget so it chains rather than going straight).
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [
        edge("ab", "a", "b"),
        edge("bc", "b", "c"),
        edge("ca", "c", "a"),
        edge("ac", "a", "c"),
    ];
    let t = index_model(&n, &e).expect("fits");
    let geometry = run(&t, 1.0).expect("never fails");
    assert!(matches!(geometry.nodes, NodeGeometry::Point { .. }));
    let EdgeGeometry::Polyline(paths) = &geometry.edges else {
        panic!("expected Polyline, got {:?}", geometry.edges);
    };
    // ca (reversed, so a->c in layer space) and ac both span layers 0..2: each
    // owns one dummy point of its own (no dedup across parallel arcs, see
    // `layering.rs`'s `preds_succs` doc).
    assert_eq!(paths.offsets, [0, 0, 0, 1, 2]);
    let codes: Vec<_> = geometry.notes.iter().map(|note| note.code).collect();
    assert_eq!(codes, [NoteCode::EdgeReversed], "{codes:?}");
    assert_eq!(
        crossings_for(&t),
        0,
        "one vertex per layer: nothing to cross"
    );
}

/// The graph [`every_edge_draws_one_dummy_per_layer_it_skips`] builds: eight nodes
/// `a..h`, node index = position, eight edges in the order listed. `b -> f` is spelled
/// twice and the two copies are **not** adjacent (edge 0 and edge 5), so the one arc they
/// share has a member list that no `Range<u32>` over edge indices can spell; `h -> a` is
/// the reversed three-layer skip edge that range used to swallow.
const SPLIT_PAIR_GRAPH: [(&str, &str, &str); 8] = [
    ("bf", "b", "f"),
    ("ac", "a", "c"),
    ("ad", "a", "d"),
    ("af", "a", "f"),
    ("ha", "h", "a"),
    ("bf2", "b", "f"),
    ("ce", "c", "e"),
    ("eh", "e", "h"),
];

/// A unit twin of `snapshot_cmd::dag::invariants`, on the smallest graph that reproduces
/// roundtrip seed 66's failure (`docs/measurements/fix-dag-roundtrip.md`): an edge's
/// interior points are one per layer it skips, so `dummies == |span| - 1` on every
/// non-loop edge, reversed ones included. Seed 66 drew `edge 44 (33 -> 1): y 3 -> 1
/// through 0 dummies, reversed=true`: arc `(20, 28)` held edges 38 and 98, its member
/// "range" `38..99` covered every edge in between, and arc `(1, 6)`'s one-layer
/// `Route::Direct` landed on `edge 4` over three layers.
#[test]
fn every_edge_draws_one_dummy_per_layer_it_skips() {
    let ids = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let n = ids.map(|id| node(id, ""));
    let e = SPLIT_PAIR_GRAPH.map(|(id, s, t)| edge(id, s, t));
    let topology = index_model(&n, &e).expect("fits");
    let geometry = run(&topology, 1.0).expect("never fails");
    let NodeGeometry::Point { y, .. } = &geometry.nodes else {
        panic!("Point nodes")
    };
    let EdgeGeometry::Polyline(paths) = &geometry.edges else {
        panic!("Polyline edges")
    };
    let at = |id: &str| ids.iter().position(|x| *x == id).expect("known id");
    for (edge, (_, s, t)) in SPLIT_PAIR_GRAPH.iter().enumerate() {
        let (s, t) = (at(s), at(t));
        let dummies = (paths.offsets[edge + 1] - paths.offsets[edge]) as i32;
        let span = (y[t] - y[s]).abs() as i32;
        assert_eq!(
            span,
            dummies + 1,
            "edge {edge} ({s} -> {t}): {dummies} dummies over {span} layers"
        );
    }
}

#[test]
fn run_is_deterministic() {
    let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let e = [
        edge("ac", "a", "c"),
        edge("bc", "b", "c"),
        edge("cd", "c", "d"),
    ];
    let t = index_model(&n, &e).expect("fits");
    assert_eq!(run(&t, 1.0), run(&t, 1.0));
}

#[test]
fn the_stage_is_registered_under_its_id_and_scales_y_by_its_spacing() {
    assert_eq!(Sugiyama::ID, "layout.dag.sugiyama");
    assert_eq!(SugiyamaParams::default().layer_spacing, 1.0);
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
    let t = index_model(&n, &e).expect("fits");
    let wide = SugiyamaParams { layer_spacing: 2.5 };
    let NodeGeometry::Point { y, .. } = Sugiyama::run(&t, &wide).expect("runs").nodes else {
        panic!("Point nodes");
    };
    assert_eq!(y, [0.0, 2.5, 5.0]);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let err = Sugiyama::run(&t, &SugiyamaParams { layer_spacing: bad });
        let want = StageError::Param {
            name: "layer_spacing",
            rule: "finite and above 0",
        };
        assert_eq!(err, Err(want), "{bad}");
    }
}
