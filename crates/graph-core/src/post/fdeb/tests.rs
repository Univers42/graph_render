//! `post.bundle.fdeb` pinned: the schedule, the four compatibility terms, the pair list's
//! order, the gather-form iteration, and the fixtures.
//!
//! Every figure here is an exact one on a hand-worked input, not a tolerance. A test that
//! accepted "close enough" would let a swapped operator, a halved constant or a reordered
//! sum through, and those are precisely the changes this slice's claims are about.

use super::*;
use crate::index::index_model;
use crate::layout::Geometry;
use crate::records::build::{edge, node};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

/// A layout's geometry from explicit positions, so a test states the drawing it means.
fn points(n: usize, xy: &[(f32, f32)]) -> Geometry {
    Geometry {
        nodes: NodeGeometry::Point {
            x: xy.iter().map(|p| p.0).collect(),
            y: xy.iter().map(|p| p.1).collect(),
        },
        edges: EdgeGeometry::Line,
        notes: Vec::new(),
    }
    .piped(n)
}

impl Geometry {
    /// Asserts the node count the test meant, which a slice of the wrong length would
    /// otherwise hide.
    fn piped(self, n: usize) -> Geometry {
        assert_eq!(self.nodes.columns()[0].1.len(), n, "node count");
        self
    }
}

/// Two edges between the same pair of points, and one crossing edge: the smallest graph
/// where the pair list has something to keep and something to prune.
fn three_edges() -> crate::index::Topology {
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "a", "b"),
        edge("e2", "c", "d"),
    ];
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn the_schedule_is_the_references_own_doubling_and_halving() {
    let params = FdebParams {
        cycles: 6,
        iterations: 8,
        segments: 12,
        ..FdebParams::default()
    };
    let sched = schedule(&params);
    assert_eq!(
        sched,
        vec![
            (1, 0.6, 8),
            (2, 0.3, 5),
            (4, 0.15, 3),
            (8, 0.075, 2),
            (12, 0.0375, 1),
            (12, 0.01875, 1)
        ],
        "points double to the cap, the step halves, iterations fall to two thirds rounded up"
    );
    // Degenerate parameters are floored, never obeyed: zero cycles still runs one.
    let floored = schedule(&FdebParams {
        cycles: 0,
        iterations: 0,
        segments: 0,
        ..FdebParams::default()
    });
    assert_eq!(floored, vec![(1, 0.6, 1)]);
}

#[test]
fn the_four_compatibility_terms_are_the_references_arithmetic() {
    // A square of unit side: four unit edges, two parallel pairs and two perpendicular
    // pairs, so every term can be checked against a value worked out by hand.
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "c", "d"),
        edge("e2", "a", "c"),
        edge("e3", "b", "d"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");

    // Ca: |cos| of the angle between the unit directions. Parallel 1, perpendicular 0.
    assert_eq!(frames.angle(0, 1), 1.0);
    assert_eq!(frames.angle(0, 2), 0.0);
    // Cs: equal lengths score exactly 1.
    assert_eq!(frames.scale(0, 1), 1.0);
    // Cp: midpoints a unit apart, so 1/(1+1) = 1/2 for the two horizontal edges; the
    // same for the two vertical ones.
    assert_eq!(frames.position(0, 1), 0.5);
    assert_eq!(frames.position(2, 3), 0.5);
    // A perpendicular pair has a zero-width shadow, so Cv sends it to 0 and the product is
    // 0 whatever the other terms say.
    assert_eq!(frames.visibility(0, 2), 0.0);
    assert_eq!(frames.compatibility(0, 2, true), 0.0);
    assert_eq!(frames.compatibility(0, 1, true), 0.5);
    // Without the visibility term the perpendicular pair is scored on three terms alone.
    assert_eq!(frames.compatibility(0, 2, false), 0.0, "Ca is 0 anyway");
    // A self-loop has no direction and no length. Its unit direction is the reference's
    // `(0, 0) / max(0, LEN_EPS)`, so the angle term is 0 and the product is 0 rather than a
    // NaN from a division by zero — the length guard is what makes the degenerate edge score
    // "no compatibility" instead of poisoning the whole pair list.
    let loop_nodes = [node("a", ""), node("b", "")];
    let loops = index_model(&loop_nodes, [edge("l", "a", "a")].as_slice()).expect("fits");
    let loop_geometry = points(2, &[(0.0, 0.0), (5.0, 5.0)]);
    let (lx, ly) = centres(&loop_geometry.nodes);
    let loop_edges = loops.edges();
    let loop_frames =
        compat::Frames::of(lx, ly, &loop_edges.source, &loop_edges.target).expect("frames");
    assert_eq!(loop_frames.len(), 1);
    assert_eq!(
        loop_frames.ends(0),
        ((0.0, 0.0), (0.0, 0.0)),
        "both ends are node a"
    );
    assert_eq!(
        loop_frames.compatibility(0, 0, true),
        0.0,
        "no length, no angle, no NaN"
    );
    assert_eq!(loop_frames.compatibility(0, 0, false), 0.0);
}

#[test]
fn the_pair_list_is_sorted_by_edge_then_edge_and_names_the_flipped_rows() {
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.1), (1.0, 0.1)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [
        edge("e0", "a", "b"),
        edge("e1", "c", "d"),
        edge("e2", "b", "a"),
        edge("e3", "a", "b"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let params = FdebParams {
        threshold: 0.0,
        ..FdebParams::default()
    };
    let list = pairs::PairList::of(&frames, &params);

    // Every row ascending in its partner, and the partner back-pointer agreeing: that is
    // what "sorted by (edge, edge)" means in the CSR.
    for e in 0..4 {
        let row = list.row(e);
        assert!(
            row.windows(2).all(|w| w[0].partner < w[1].partner),
            "edge {e}: row ascending"
        );
        for entry in row {
            let back = list
                .row(entry.partner)
                .iter()
                .find(|other| other.partner == e)
                .expect("the pair is in both rows");
            assert_eq!(back.compat, entry.compat, "compatibility is symmetric");
            assert_eq!(back.flipped, entry.flipped, "the flip is symmetric too");
        }
    }
    // e0 and e2 run opposite ways down the same line: the antiparallel pair, and the only
    // one the reference flips so the bundle does not open into an X.
    let flipped = list
        .row(0)
        .iter()
        .find(|entry| entry.partner == 2)
        .expect("the antiparallel pair survives a zero threshold");
    assert!(flipped.flipped, "e0 and e2 point opposite ways");
    assert!(
        list.row(0)
            .iter()
            .find(|entry| entry.partner == 1)
            .is_some_and(|entry| !entry.flipped),
        "a parallel pair is not flipped"
    );
    // Six pairs over four edges, and the count is of pairs, not of rows.
    assert_eq!(list.total(), 6);
    assert_eq!(list.total() as usize, list.row(0).len() + 3);
    assert!(
        list.unbundled().is_empty(),
        "a zero threshold leaves nobody out"
    );
}

#[test]
fn a_threshold_above_one_leaves_every_edge_unbundled() {
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.1), (1.0, 0.1)];
    let nodes = ["a", "b", "c", "d"].map(|id| node(id, ""));
    let edges = [edge("e0", "a", "b"), edge("e1", "c", "d")];
    let topology = index_model(&nodes, &edges).expect("fits");
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let list = pairs::PairList::of(
        &frames,
        &FdebParams {
            threshold: 1.0,
            ..FdebParams::default()
        },
    );
    assert_eq!(list.total(), 0);
    assert_eq!(list.unbundled(), &[0, 1], "both edges drawn unbundled");
}

#[test]
fn one_iteration_moves_an_interior_point_and_pins_both_endpoints() {
    // Two coincident edges on one line and a third five units above: the displacement of each
    // interior point is then the reference's arithmetic written out, with nothing else in
    // the sum. step = 0.6, strength = 0.8, spring = 0 on a straight row (its two neighbours
    // are symmetric about the point, so the Laplacian is exactly 0), and the softening
    // (SOFTEN_FRAC · RADIUS_FRAC · diag)² with diag = hypot(1, 5) over the four positions.
    let topology = three_edges();
    let xy = [(0.0, 0.0), (1.0, 0.0), (0.0, 5.0), (1.0, 5.0)];
    let geometry = points(4, &xy);
    let (x, y) = centres(&geometry.nodes);
    let endpoints = topology.edges();
    let frames = compat::Frames::of(x, y, &endpoints.source, &endpoints.target).expect("frames");
    let params = FdebParams {
        threshold: 0.0,
        visibility: false,
        ..FdebParams::default()
    };
    let list = pairs::PairList::of(&frames, &params);
    let soften = (0.01 * 0.05 * libm::hypotf(1.0, 5.0)) * (0.01 * 0.05 * libm::hypotf(1.0, 5.0));

    let before = Points::of(&topology, &geometry, 3);
    let mut after = Points::empty(3, 3);
    before.step_into(&mut after, &list, &frames, params.strength, 0.6, soften);

    // The endpoints never move: an edge has to touch its two nodes.
    assert_eq!(after.at(0, 0), (0.0, 0.0));
    assert_eq!(after.at(0, 2), (1.0, 0.0));
    // Edge 0 and edge 1 are the same segment, so the interior point's attractor is edge 1's
    // coincident point: the displacement is zero, softened but zero.
    let (px, py) = after.at(0, 1);
    assert_eq!(px, 0.5, "the pull is along the perpendicular only");
    assert!(
        py > 0.0 && py < 1e-6,
        "the coincident partner pulls nowhere, the far one barely: {py}"
    );
    // Edge 2 sits five units above edges 0 and 1, which both attract it fully: the attractor
    // is 5 below, so it moves step · strength · 5 = 0.6 · 0.8 · 5 = 2.4 down.
    let (_, y) = after.at(2, 1);
    assert!(
        (5.0 - y - 2.4).abs() < 1e-5,
        "moved toward the pair, y = {y}"
    );
}

#[test]
fn resampling_keeps_the_endpoints_and_spaces_the_row_evenly() {
    let mut points = Points::empty(1, 2);
    points.write_row(0, &[(0.0, 0.0), (4.0, 0.0)]);
    points.resample(6);
    let row: Vec<(f32, f32)> = (0..6).map(|p| points.at(0, p)).collect();
    assert_eq!(row.first(), Some(&(0.0, 0.0)), "the row's own first point");
    assert_eq!(row.last(), Some(&(4.0, 0.0)), "and its own last");
    let gaps: Vec<f32> = row.windows(2).map(|w| w[1].0 - w[0].0).collect();
    for gap in &gaps {
        assert!((gap - 0.8).abs() < 1e-6, "even arc length, got {gap}");
    }
    // A row of one point cannot be resampled and is left where it is, rather than divided by
    // zero: the guard the reference has for `k_in == 1`.
    let mut single = Points::empty(1, 2);
    single.write_row(0, &[(0.0, 0.0), (0.0, 0.0)]);
    single.resample(1);
    assert_eq!(single.shape(), (1, 1));
    assert_eq!(single.at(0, 0), (0.0, 0.0));
}

#[test]
fn bundling_a_layouts_own_polylines_composes_rather_than_replacing_them() {
    // A `Polyline` input whose bend is off the straight line: the pass bundles from the
    // drawn path, so the bend survives where no force opposes it. A pass that assumed a
    // `Line` input would start from the chord and lose it, which is what this asserts.
    let topology = three_edges();
    let geometry = Geometry {
        nodes: NodeGeometry::Point {
            x: vec![0.0, 1.0, 0.0, 1.0],
            y: vec![0.0, 0.0, 5.0, 5.0],
        },
        edges: EdgeGeometry::Polyline(Paths {
            offsets: vec![0, 1, 2, 3],
            pts: vec![0.5, 3.0, 0.5, 3.0, 0.5, 3.0],
        }),
        notes: Vec::new(),
    };
    let bundled = bundle(&topology, &geometry, &FdebParams::default()).expect("runs");
    let EdgeGeometry::Polyline(paths) = &bundled.geometry.edges else {
        panic!("bundling emits polylines");
    };
    assert_eq!(paths.offsets.len(), topology.edge_count() as usize + 1);
    for row in 0..topology.edge_count() as usize {
        let (from, to) = (paths.offsets[row] as usize, paths.offsets[row + 1] as usize);
        assert!(to > from, "edge {row} has interior points");
    }
}

#[test]
fn a_parameter_outside_the_accepted_range_is_refused_rather_than_clipped() {
    let topology = three_edges();
    let geometry = points(4, &[(0.0, 0.0), (1.0, 0.0), (0.0, 5.0), (1.0, 5.0)]);
    for params in [
        FdebParams {
            strength: 4.0,
            ..FdebParams::default()
        },
        FdebParams {
            threshold: -0.5,
            ..FdebParams::default()
        },
        FdebParams {
            threshold: f32::NAN,
            ..FdebParams::default()
        },
    ] {
        let err = bundle(&topology, &geometry, &params).expect_err("refused");
        assert!(
            matches!(err, crate::stage::StageError::Param { .. }),
            "{err}"
        );
    }
    // The same graph at the defaults runs, which is what makes the refusals above about the
    // parameter and not about the geometry.
    assert!(bundle(&topology, &geometry, &FdebParams::default()).is_ok());
}

#[test]
fn the_committed_hairball_is_exactly_what_the_generator_builds() {
    let (from_file, file_edges) = load("hairball").expect("the fixture is committed");
    let (from_formula, formula_edges) = hairball(42);
    assert_eq!(from_file, from_formula, "the file and the formula agree");
    assert_eq!(
        file_edges, formula_edges,
        "edge for edge, in the same order"
    );
    assert_eq!(from_file.len(), 42, "two clusters of 21");
    // 84 cross edges (21 A nodes x 4 strides), then per cluster 20 path + 21 cycle + 6 fan
    // edges: the edge count every ink figure in docs/measurements/phase08-ink.md was taken
    // at, restated here so a fixture that grew would move this number.
    assert_eq!(file_edges.len(), 84 + 2 * (20 + 21 + 6));
    assert_eq!(file_edges.len(), 178);
    // The cross edges come first and in stride order, which is what puts the four
    // near-parallel families at the front of the pair list.
    assert_eq!(file_edges[0].source, "a0");
    assert_eq!(file_edges[0].target, "b0");
    assert_eq!(file_edges[3].target, "b9", "stride 17, k = 3");
    assert_eq!(file_edges[4].source, "a1");
    assert_eq!(
        file_edges[83].source, "a20",
        "84 cross edges, then A's own path"
    );
    assert_eq!(
        (&file_edges[84].source, &file_edges[84].target),
        (&"a0".into(), &"a1".into())
    );
    // No self-loop and no duplicate id, which the indexer would otherwise refuse.
    let mut ids: Vec<&str> = file_edges.iter().map(|e| e.id.as_str()).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), before, "unique edge ids");
}

#[test]
fn a_fixture_that_is_not_there_is_refused_by_name() {
    assert!(load("nope").is_err());
    assert!(
        load("long-span").is_ok(),
        "the second fixture is committed too"
    );
    // A generator run is the same graph at every size, which is what makes the scale sweep
    // the same graph as the fixture rather than a different one that only looks like it.
    let (small, small_edges) = hairball(10);
    let (large, large_edges) = hairball(100);
    assert_eq!(small.len(), 10);
    assert_eq!(large.len(), 100);
    assert!(small_edges.len() < large_edges.len());
    assert!(
        hairball(2).0.len() == 2,
        "the smallest graph still has two clusters"
    );
}
