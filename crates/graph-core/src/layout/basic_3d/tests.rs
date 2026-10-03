//! What the four graph-free 3D placements share, checked once so each of their own test
//! files can be about its own constants.
//!
//! The per-placement tests live next to their modules — [`sphere`](super::sphere),
//! [`helix`](super::helix), [`cube`](super::cube), [`spiral`](super::spiral) — because
//! that is where the reference's formulas are and a reader checking this port against
//! `basic.py` wants them in the same file. What is here is the few claims that hold for all
//! of them and are not a particular formula's business: they read the node count and
//! nothing else, they emit a 3D geometry or nothing, and they publish four distinct ids.

use super::SCALE;
use crate::index::{Topology, index_model};
use crate::layout::Geometry;
use crate::layout::basic_3d::{cube, helix, sphere, spiral};
use crate::records::NodeRecord;
use crate::records::build::{edge, node};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::snapshot::Dim;

/// `count` nodes and no edges. These four read nothing but the count, so an edgeless
/// topology is the honest input and every test below uses it.
pub(super) fn bare(count: u32) -> Topology {
    let records: Vec<NodeRecord> = (0..count).map(|i| node(&format!("n{i}"), "")).collect();
    index_model(&records, &[]).expect("fits")
}

/// `x`, `y`, `z`, or a panic: every layout here is `Point` in space, and a 2D geometry
/// would answer all three wrongly and quietly.
pub(super) fn space(g: &Geometry) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    let Some(z) = g.z.as_ref() else {
        panic!("a z column: this layout is 3D");
    };
    (x.clone(), y.clone(), z.clone())
}

/// The golden angle `pi*(3 - sqrt(5))`, recomputed here independently of
/// [`sphere`](super::sphere)'s own constant so a drift in one is caught by the other.
pub(super) fn golden() -> f64 {
    core::f64::consts::PI * (3.0 - f64::sqrt(5.0))
}

/// One of the four placements: its id and the `run` the registry registers for it.
type Placement = (
    &'static str,
    fn(&Topology) -> Result<Geometry, crate::stage::StageError>,
);

/// Every one of the four, as a list of `(id, run)` so one assertion covers all of them.
fn all() -> [Placement; 4] {
    [
        (sphere::ID, sphere),
        (helix::ID, helix),
        (cube::ID, cube),
        (spiral::ID, spiral),
    ]
}

/// **They read the node count and nothing else.** `basic.py:22`, `:65`, `:83` and `:36`
/// take `(num_nodes, scale)` and no graph at all, so every edge is ignored: a graph and its
/// edgeless twin draw identically.
///
/// This is the one property a reader is most likely to mistake for a bug — "the layout
/// does not look at the graph" — and it is the reference's own behaviour, so it is
/// asserted rather than left to be discovered.
#[test]
fn these_four_read_the_node_count_and_never_the_edges() {
    let records: Vec<NodeRecord> = (0..6).map(|i| node(&format!("n{i}"), "")).collect();
    let edges = [
        edge("e0", "n0", "n1"),
        edge("e1", "n1", "n2"),
        edge("e2", "n5", "n0"),
    ];
    let plain = index_model(&records, &[]).expect("fits");
    let wired = index_model(&records, &edges).expect("fits");
    assert_eq!(plain.edge_count(), 0);
    assert_eq!(wired.edge_count(), 3);
    for (id, run) in all() {
        assert_eq!(
            run(&plain).expect("runs"),
            run(&wired).expect("runs"),
            "{id} changed the drawing when edges were added"
        );
    }
}

/// All four emit `Point` nodes, straight `Line` edges, no notes, and **a z column** —
/// at every size from empty to two nodes past the corner count.
///
/// The z column is the assertion that matters: a `Geometry::planar` in any of these four
/// would hash, decode, and answer a 3D question with a 2D drawing.
#[test]
fn all_four_emit_point_line_a_z_column_and_no_notes() {
    for (id, run) in all() {
        for n in [0u32, 1, 2, 8, 9, 33] {
            let g = run(&bare(n)).expect("runs");
            assert_eq!(g.edges, EdgeGeometry::Line, "{id} n={n}");
            assert!(g.notes.is_empty(), "{id} n={n} carries notes");
            assert_eq!(g.dim(), Dim::D3, "{id} n={n} was labelled 2D");
            let (x, y, z) = space(&g);
            let want = n as usize;
            assert_eq!(
                (x.len(), y.len(), z.len()),
                (want, want, want),
                "{id} n={n}"
            );
            assert!(
                x.iter().chain(&y).chain(&z).all(|v| v.is_finite()),
                "{id} n={n}"
            );
        }
    }
}

/// An empty graph is an empty **3D** geometry, not a 2D one. The column's *presence* is
/// the dimension, so `n = 0` must not drop it — a layout that returned `planar` for the
/// empty case and `in_space` otherwise would be self-consistent and wrong.
#[test]
fn an_empty_graph_is_still_a_3d_geometry_for_all_four() {
    for (id, run) in all() {
        let g = run(&bare(0)).expect("runs");
        assert_eq!(g.dim(), Dim::D3, "{id} dropped the z column when empty");
        assert_eq!(g.z.as_deref(), Some([].as_slice()), "{id}");
        let NodeGeometry::Point { x, y } = &g.nodes else {
            panic!("{id} point nodes");
        };
        assert_eq!((x.len(), y.len()), (0, 0), "{id}");
    }
}

/// Four distinct ids, so four distinct hash-gate stages. Two layouts sharing an id would
/// make the gate unable to tell two different drawings apart, and `layout.force.spring` /
/// `layout.force.spring3d` is the precedent for why that matters (the same drawing, two
/// dimensions, two ids, two digests).
#[test]
fn the_four_capability_ids_are_distinct_and_named() {
    let mut ids: Vec<&str> = all().iter().map(|(id, _)| *id).collect();
    assert_eq!(
        ids.clone(),
        vec![
            "layout.basic3d.sphere",
            "layout.basic3d.helix",
            "layout.basic3d.cube",
            "layout.basic3d.spiral"
        ]
    );
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 4, "two of the four ids collide");
}

/// Each is a pure function of its input: the same topology twice gives the same three
/// columns, byte for byte. `SPHERE` and `HELIX` are closed form and this is trivially
/// true; `CUBE`'s scatter is drawn from a **fixed** stream, so this is a real claim about
/// the seed rather than about arithmetic.
#[test]
fn each_is_deterministic_over_the_same_topology() {
    for (id, run) in all() {
        let topology = bare(97);
        assert_eq!(
            run(&topology).expect("runs"),
            run(&topology).expect("runs"),
            "{id} is not run-to-run deterministic"
        );
    }
}

/// Every **coordinate** of every node is within `[-scale, +scale]` — the one bound all
/// four share.
///
/// It is per-axis and not `sqrt(x^2 + y^2 + z^2)` for a reason that is a fact about the
/// drawings rather than a loosening: `z` is a **separate** scale of its own in all four.
/// The helix climbs the whole `[-scale, +scale]` while staying on a 1.5-wide circle, so its
/// head sits 5.22 from the origin; the cube's corners are at `+-scale` on all three
/// axes at once, so a corner is `sqrt(3)*scale` = 8.66 out; and the spiral's radius climbs
/// from `scale/2` to `scale` while its `z` climbs the same `[-scale, +scale]`, so no single
/// node is more than `scale` from the origin on any axis. A 3D-norm bound would be
/// asserting the reference is wrong. The per-axis extent each module draws to is asserted
/// in that module's own tests.
#[test]
fn every_coordinate_is_within_the_scale() {
    for (id, run) in all() {
        let n = 200u32;
        let (x, y, z) = space(&run(&bare(n)).expect("runs"));
        for i in 0..n as usize {
            for (axis, value) in [x[i], y[i], z[i]].into_iter().enumerate() {
                assert!(
                    f64::from(value).abs() <= SCALE * 1.001,
                    "{id} node {i} axis {axis} is at {value}, outside the scale"
                );
            }
        }
    }
}

/// The four ids are the ones the ledger and the coverage table use, spelled out here so a
/// rename has to break this file rather than silently change a stage name.
/// The four ids are the ones the ledger and the coverage table use, spelled out here so a
/// rename has to break this file rather than silently change a stage name.
mod scale;

#[test]
fn the_ids_are_the_scigraphs_names() {
    assert_eq!(sphere::ID, "layout.basic3d.sphere", "SciGraphs SPHERE");
    assert_eq!(helix::ID, "layout.basic3d.helix", "SciGraphs HELIX");
    assert_eq!(cube::ID, "layout.basic3d.cube", "SciGraphs CUBE");
    assert_eq!(spiral::ID, "layout.basic3d.spiral", "SciGraphs SPIRAL_3D");
    for id in [sphere::ID, helix::ID, cube::ID, spiral::ID] {
        assert!(id.starts_with("layout."), "{id} has no layout. prefix");
    }
}
