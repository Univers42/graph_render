use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::notes::NoteCode;

/// One column as the bits the wire carries: a transpose is exact only if it moves the value
/// itself, and two `f32` that compare equal can still be different numbers.
fn bits(column: &[f32]) -> Vec<u32> {
    column.iter().map(|value| value.to_bits()).collect()
}

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

/// The `horizontal` parameter is the whole of the change: the same pipeline, the same
/// numbers, with x and y exchanged. `a -> c` spans two layers, so its path carries an interior
/// point and the swap has something to move. `docs/decisions/dag-horizontal.md` condition 1.
#[test]
fn horizontal_draws_the_layers_along_x_bit_for_bit() {
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [
        edge("ab", "a", "b"),
        edge("bc", "b", "c"),
        edge("ac", "a", "c"),
    ];
    let t = index_model(&n, &e).expect("fits");
    let vertical = Sugiyama::run(&t, &SugiyamaParams::default()).expect("runs");
    let horizontal = Sugiyama::run(
        &t,
        &SugiyamaParams {
            horizontal: true,
            ..SugiyamaParams::default()
        },
    )
    .expect("runs");
    let NodeGeometry::Point { x: vx, y: vy } = &vertical.nodes else {
        panic!("Point nodes");
    };
    let NodeGeometry::Point { x: hx, y: hy } = &horizontal.nodes else {
        panic!("Point nodes");
    };
    assert_eq!(bits(hx), bits(vy), "the horizontal x is the vertical y");
    assert_eq!(bits(hy), bits(vx), "the horizontal y is the vertical x");
    let EdgeGeometry::Polyline(turned) = &horizontal.edges else {
        panic!("Polyline edges");
    };
    let EdgeGeometry::Polyline(straight) = &vertical.edges else {
        panic!("Polyline edges");
    };
    assert!(
        !straight.pts.is_empty(),
        "every edge here spans a layer, so the paths have interior points"
    );
    assert_eq!(
        turned.offsets, straight.offsets,
        "the CSR shape is not a coordinate"
    );
    assert_eq!(turned.pts.len(), straight.pts.len());
    for (pair, plain) in turned
        .pts
        .as_chunks::<2>()
        .0
        .iter()
        .zip(straight.pts.as_chunks::<2>().0)
    {
        assert_eq!(
            bits(pair),
            bits(&[plain[1], plain[0]]),
            "x takes y and y takes x"
        );
    }
}

/// `false` is the drawing that was there before the parameter existed: the default run and an
/// explicit `horizontal: false` are one answer, so the flag cannot move a byte at its default.
#[test]
fn horizontal_false_is_the_default_drawing() {
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
    let t = index_model(&n, &e).expect("fits");
    assert_eq!(
        Sugiyama::run(&t, &SugiyamaParams::default()),
        Sugiyama::run(
            &t,
            &SugiyamaParams {
                horizontal: false,
                ..SugiyamaParams::default()
            }
        )
    );
}

#[test]
fn the_stage_is_registered_under_its_id_and_scales_y_by_its_spacing() {
    assert_eq!(Sugiyama::ID, "layout.dag.sugiyama");
    assert_eq!(SugiyamaParams::default().layer_spacing, 1.0);
    let n = ["a", "b", "c"].map(|id| node(id, ""));
    let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
    let t = index_model(&n, &e).expect("fits");
    let wide = SugiyamaParams {
        layer_spacing: 2.5,
        horizontal: false,
    };
    let NodeGeometry::Point { y, .. } = Sugiyama::run(&t, &wide).expect("runs").nodes else {
        panic!("Point nodes");
    };
    assert_eq!(y, [0.0, 2.5, 5.0]);
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let err = Sugiyama::run(
            &t,
            &SugiyamaParams {
                layer_spacing: bad,
                horizontal: false,
            },
        );
        let want = StageError::Param {
            name: "layer_spacing",
            rule: "finite and above 0",
        };
        assert_eq!(err, Err(want), "{bad}");
    }
}
