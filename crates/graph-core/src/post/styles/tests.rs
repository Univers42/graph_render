//! The generators, pinned value by value — every style, every branch, every parameter
//! refusal. Split into its own file to keep `styles.rs` under the house line limit.
//!
//! Every expectation below is stated in `f64` over exactly representable inputs — centres
//! at whole numbers, `curvature` 0.5, `parallel_offset` 0.25, radius 0.25 — so each pinned
//! value is the arithmetic itself and not a rounding artefact. A `f32` literal would
//! hide a swapped operator behind a rounding coincidence; a `f64` one cannot, because
//! every step here is exact.

use super::fan::fan;
use super::*;
use crate::edgekind::EdgeKind;
use crate::index::Topology;
use crate::{EdgeRecord, NodeKind, NodeRecord, index_model};
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, Paths};
use graph_contract::snapshot::SnapshotError;

mod contract;
mod shapes;

/// The bend every pin below uses, 0.5 in binary — exact, so the bulge is exact.
const CURVATURE: f32 = 0.5;
/// The parallel gap every pin below uses, 0.25 in binary — exact, so the fan is exact.
const GAP: f32 = 0.25;
/// A self-loop's radius every pin below uses, 0.25 in binary — exact, so the centre's
/// half-radius lift is exact.
const RADIUS: f32 = 0.25;
/// Tighter than `f32` can resolve over this range, so it only ever catches a real
/// geometric difference and never a representation one.
const EXACT: f64 = 1e-9;
/// The tolerance the loop's `libm` vertices are checked at, and the radius they must sit
/// at. `f32` holds about seven digits, so 1e-6 is a representation-sized slack and no
/// more.
const LOOP: f64 = 1e-6;

fn node(id: &str) -> NodeRecord {
    NodeRecord {
        kind: NodeKind::Record,
        database_id: None,
        source: "test".into(),
        label: id.into(),
        group: None,
        weight: 1.0,
        version: 0.0,
        has_note: false,
        icon: None,
        id: id.into(),
    }
}

fn edge(id: &str, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        kind: EdgeKind::Relation,
        child_first: false,
        label: "relates_to".into(),
        strength: 1.0,
        directed: true,
        record_id: None,
    }
}

/// A topology over node ids in the order given, so the dense indices are 0, 1, 2, ...
/// and "the lower endpoint of a pair" is a testable thing.
fn topology(ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<NodeRecord> = ids.iter().map(|id| node(id)).collect();
    let records: Vec<EdgeRecord> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
    index_model(&nodes, &records).expect("a test topology indexes")
}

/// `centres` as `Point` geometry.
fn points(centres: &[(f32, f32)]) -> NodeGeometry {
    NodeGeometry::Point {
        x: centres.iter().map(|c| c.0).collect(),
        y: centres.iter().map(|c| c.1).collect(),
    }
}

/// `style`'s pinned parameters, with the exact-input constants above.
fn params(style: Style) -> StyleParams {
    StyleParams {
        style,
        orthogonal: Orthogonal::Z,
        curvature: CURVATURE,
        parallel_offset: GAP,
        self_loop_radius: RADIUS,
        self_loop_segments: 8,
    }
}

/// The `Paths` `params` produced over `t`, or a panic naming the case.
fn paths(t: &Topology, centres: &[(f32, f32)], params: &StyleParams) -> Paths {
    let edges = style_edges(t, &points(centres), params).expect("the parameters are legal");
    match edges {
        EdgeGeometry::Polyline(paths) | EdgeGeometry::Curve { paths, .. } => paths,
        EdgeGeometry::Line => panic!("{:?} stored no rows", params.style),
    }
}

/// Row `e`'s interior points as the `f64` pairs the generators actually computed,
/// before the single `f32` cast — so a pinned value is compared without any rounding in
/// the way.
fn row(paths: &Paths, e: u32) -> Vec<(f64, f64)> {
    let e = e as usize;
    let (from, to) = (paths.offsets[e] as usize, paths.offsets[e + 1] as usize);
    (from..to)
        .map(|p| (f64::from(paths.pts[2 * p]), f64::from(paths.pts[2 * p + 1])))
        .collect()
}

/// The same rows, checked against `f64` expectations within [`EXACT`]. `f32` can hold a
/// whole number exactly, so these pins are exact and the slack is only there to say so.
fn expect(t: &Topology, centres: &[(f32, f32)], p: &StyleParams, want: &[Vec<(f64, f64)>]) {
    let got = paths(t, centres, p);
    let read: Vec<Vec<(f64, f64)>> = (0..t.edge_count()).map(|e| row(&got, e)).collect();
    for (e, (row, want)) in read.iter().zip(want).enumerate() {
        assert_eq!(row.len(), want.len(), "{:?} edge {e}: row length", p.style);
        for (i, (at, want)) in row.iter().zip(want).enumerate() {
            assert!(
                (at.0 - want.0).abs() <= EXACT && (at.1 - want.1).abs() <= EXACT,
                "{:?} edge {e} point {i}: got {at:?}, want {want:?}",
                p.style
            );
        }
    }
}

#[test]
fn a_quadratic_bulges_half_a_chord_length_per_unit_of_curvature() {
    // Chord (0,0) -> (4,0). The perpendicular is (d.y, -d.x) = (0, -4); the bulge takes
    // the -1 side because the source is not above the target's anti-diagonal
    // (0 + 0 > 4 + 0 is false), so the control point is pushed to +y, not -y.
    let t = topology(&["a", "b"], &[("e0", "a", "b")]);
    expect(
        &t,
        &[(0.0, 0.0), (4.0, 0.0)],
        &params(Style::Quadratic),
        &[vec![(2.0, 1.0)]],
    );
    // The other side, from the same chord read the other way round: perpendicular
    // (0, 4), bulge +1, and the control point is still at +y — the bend follows the
    // chord's diagonal, not the edge's direction.
    let t = topology(&["a", "b"], &[("e0", "b", "a")]);
    expect(
        &t,
        &[(0.0, 0.0), (4.0, 0.0)],
        &params(Style::Quadratic),
        &[vec![(2.0, 1.0)]],
    );
}
