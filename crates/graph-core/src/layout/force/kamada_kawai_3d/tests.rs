//! `layout.force.kamada_kawai_3d`: the sphere start, the 3x3 solve, and the fixture that breaks
//! igraph.
//!
//! Nothing here re-tests the algorithm — `super::super::kamada_kawai::tests` does that over the
//! identical descent at `D = 2` and pins those coordinates bit for bit. What these tests exist for
//! is the 3D-only surface: the deterministic sphere start, the Cramer solve, and the reference's
//! own failure on a three-node graph.

use super::super::kamada_kawai::{KK_CEILING, KamadaKawai, KkParams};
use super::{ID_3D, KamadaKawai3D};
use crate::index::{Topology, empty_model};
use crate::layout::Geometry;
use crate::layout::coords::probe::graph;
use crate::registry;
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

/// The `n`-node path, in the vertex order the fixture set uses.
fn path(n: u32) -> Topology {
    graph(
        n,
        &(0..n.saturating_sub(1))
            .map(|i| (i, i + 1))
            .collect::<Vec<_>>(),
    )
}

fn columns(g: &Geometry) -> (&[f32], &[f32], &[f32]) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    (
        x.as_slice(),
        y.as_slice(),
        g.z.as_deref().expect("a 3D geometry carries a z column"),
    )
}

fn space(t: &Topology, params: &KkParams) -> Geometry {
    KamadaKawai3D::run(t, params).expect("finite")
}

/// **The reference's own failure, on the fixture that causes it.** `gate-01` in
/// `conformance.jsonl` is three nodes with the edges `1-0` and `2-0` — the 3-vertex path, drawn
/// with its hub first — and python-igraph 0.11.9 returns **three infinite coordinates out of
/// nine** from `layout_kamada_kawai(dim=3)` on it, in all six vertex orderings. It is not a
/// component count, not an isolated node and not a degree: the graph is connected, has no
/// isolated node, and its degrees are 2, 1, 1, which `star with 2 leaves` shares and
/// `a single edge` (degrees 1, 1) does not. It is the 3x3 Newton block being near-singular at
/// three vertices, so one step overflows `f64` — and the same graph at `dim=2` is finite, because
/// the 2x2 block is not.
///
/// That overflow is why `IGRAPH_KK` compares 957 coordinates rather than 1020, and it is a
/// recorded **reference defect**: this port guards the same block, and this test holds it to
/// producing finite geometry on the very fixture the reference cannot.
#[test]
fn the_three_node_path_that_breaks_igraph_is_finite_here() {
    // gate-01: three nodes, edges 1-0 and 2-0, so the hub is vertex 0.
    let t = graph(3, &[(1, 0), (2, 0)]);
    let g = space(&t, &KkParams::default());
    let (x, y, z) = columns(&g);
    assert_eq!((x.len(), y.len(), z.len()), (3, 3, 3));
    assert!(
        x.iter().chain(y).chain(z).all(|v| v.is_finite()),
        "a non-finite coordinate would be refused with NonFinite, not returned"
    );
    // …and not merely finite: the drawing is a real one, spread over the sphere rather than
    // collapsed onto a point by a step that was clamped to nothing.
    let largest = x
        .iter()
        .chain(y)
        .chain(z)
        .fold(0.0_f32, |a, b| a.max(b.abs()));
    assert!(largest > 0.0, "the whole drawing is the origin");
    assert!(largest.is_finite(), "the drawing ran away");
}

/// The same guard on the sizes either side of it, and on the shapes igraph's row actually holds.
/// Three vertices is where the reference breaks; two is where a pair cannot move at all.
#[test]
fn small_graphs_stay_finite_in_space() {
    for (n, pairs) in [
        (0u32, Vec::new()),
        (1, Vec::new()),
        (2, vec![(0, 1)]),
        (3, vec![(0, 1), (0, 2)]),
        (4, vec![(0, 1), (0, 2), (0, 3)]),
        (5, vec![(0, 1), (1, 2), (2, 3), (3, 4)]),
    ] {
        let t = graph(n, &pairs);
        let g = space(&t, &KkParams::default());
        let (x, y, z) = columns(&g);
        assert!(
            x.iter().chain(y).chain(z).all(|v| v.is_finite()),
            "n={n} produced a non-finite coordinate"
        );
    }
    let blank = space(&empty_model(), &KkParams::default());
    assert_eq!(columns(&blank).0.len(), 0, "the empty graph drew something");
}

/// The start is the spec's sphere: both poles on the axis at radius `0.36 * sqrt(n)`, and every
/// interior vertex on the sphere. `maxiter = 0` is not a legal igraph argument, so this reads the
/// start through the port's own solve rather than through the reference.
#[test]
fn the_start_places_both_poles_on_the_axis() {
    let t = path(7);
    let start = crate::layout::force::kamada_kawai::descent::start::<3>(7);
    let radius = 0.36 * libm::sqrt(7.0);
    assert!(
        (start[0][2] + radius).abs() < 1e-12,
        "south pole: {:?}",
        start[0]
    );
    assert!(
        (start[6][2] - radius).abs() < 1e-12,
        "north pole: {:?}",
        start[6]
    );
    assert_eq!(
        (start[0][0], start[0][1]),
        (0.0, 0.0),
        "a pole is off the axis"
    );
    assert_eq!(
        (start[6][0], start[6][1]),
        (0.0, 0.0),
        "a pole is off the axis"
    );
    // Every interior vertex sits on the sphere of that radius, and none two coincide — a start
    // with two vertices together would divide by zero on the very first gradient.
    for point in &start[1..6] {
        let r = libm::sqrt(point[0] * point[0] + point[1] * point[1] + point[2] * point[2]);
        assert!((r - radius).abs() < 1e-9, "off the sphere: {r} vs {radius}");
    }
    for i in 0..7 {
        for j in (i + 1)..7 {
            let d = libm::sqrt(squared_gap(start[i], start[j]));
            assert!(d > 1e-9, "vertices {i} and {j} start on top of each other");
        }
    }
    assert!(
        t.node_count() == 7,
        "the fixture and the start disagree on n"
    );
}

/// The 3x3 solve is a different answer from the 2x2 one, because the third axis changes the
/// gradient it is fed. Were `D` ignored in the descent, these two would be equal.
#[test]
fn the_third_axis_changes_the_answer() {
    let t = path(6);
    let NodeGeometry::Point { x, y } = KamadaKawai::run(&t, &KkParams::default())
        .expect("finite")
        .nodes
    else {
        panic!("point nodes");
    };
    let solid = space(&t, &KkParams::default());
    let (x3, y3, z3) = columns(&solid);
    assert!(
        z3.iter().any(|v| *v != 0.0),
        "z is all zero: this is a 2D run"
    );
    assert_ne!(
        x, x3,
        "the 3D x column is the 2D one: z is not in the descent"
    );
    assert_ne!(
        y, y3,
        "the 3D y column is the 2D one: z is not in the descent"
    );
}

/// The id is registered apart from the 2D one, with its own hash-gate stage and the same ceiling.
#[test]
fn the_three_d_id_is_registered_apart_from_the_two_d_one() {
    let three = registry::find(ID_3D).expect("registered");
    let two = registry::find(KamadaKawai::ID).expect("registered");
    assert_ne!(three.id, two.id);
    assert_eq!(three.id, "layout.force.kamada_kawai_3d");
    assert_eq!(three.meta.stage, "layout");
    assert_eq!(three.meta.scale_ceiling, KK_CEILING);
}

/// No seed and no generator: the same input twice is the same answer. This is the one igraph
/// layout with no licence gap on its start, and the test that says so.
#[test]
fn there_is_no_randomness_so_a_run_is_repeatable() {
    let t = graph(
        9,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 0),
        ],
    );
    let once = space(&t, &KkParams::default());
    let twice = space(&t, &KkParams::default());
    assert_eq!(
        once.nodes, twice.nodes,
        "two runs of a deterministic layout differ"
    );
    assert_eq!(once.z, twice.z, "two runs of a deterministic layout differ");
}

/// The `epsilon` early stop still stops, at `D = 3`. A budget that only worked in the plane would
/// show as a `maxiter`-sized run on a graph that should have settled.
#[test]
fn epsilon_stops_the_three_d_descent_early() {
    let t = path(8);
    let stopped = space(
        &t,
        &KkParams {
            epsilon: 1e9,
            ..KkParams::default()
        },
    );
    let ran = space(&t, &KkParams::default());
    assert_ne!(
        columns(&stopped).0,
        columns(&ran).0,
        "epsilon = 1e9 did not stop the descent"
    );
}

/// `|p_i - p_j|^2` for two of the start's rows.
fn squared_gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    let mut sum = 0.0;
    for axis in 0..3 {
        sum += (a[axis] - b[axis]) * (a[axis] - b[axis]);
    }
    sum
}
