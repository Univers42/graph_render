//! `lanes`'s oracle held to real snapshots and to one perturbation per compared column.
//!
//! Condition 4 of `docs/decisions/dag-lanes.md` is the reason this file is not one test.
//! The oracle's `oracle` string claims a bit-for-bit comparison, so every column it names
//! needs a control that proves the comparison is live: node `x`, node `y`, the polyline's
//! interior points, the polyline's `offsets`, and the notes. `with_x` alone would have proved
//! only the `x` comparison. Each control perturbs an honest snapshot and must be *named* —
//! the failure has to say which column it caught, or a control that fired for the wrong
//! reason would still look green.
//!
//! The perturbed snapshots stay legal: every one is rebuilt through the constructor, so a
//! control fails on the comparison and not on a refusal.

use super::*;

/// `snapshot` with `notes` in place of its own: the only way a note can come or go.
fn renoted(snapshot: &Snapshot, notes: &[Note]) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    parts.notes = Notes::of(notes);
    Snapshot::new(parts).expect("valid")
}

/// `snapshot` with node `i` moved by `(dx, dy)`: two columns in one perturbation, and each
/// half of it is its own control below.
fn moved(snapshot: &Snapshot, i: usize, (dx, dy): (f32, f32)) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    let NodeGeometry::Point { x, y } = &mut parts.nodes else {
        panic!("Point nodes")
    };
    x[i] += dx;
    y[i] += dy;
    Snapshot::new(parts).expect("valid")
}

/// `snapshot` with one interior point's y moved by `dy`: the `pts` column only.
fn rebent(snapshot: &Snapshot, point: usize, dy: f32) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    let EdgeGeometry::Polyline(paths) = &mut parts.edges else {
        panic!("Polyline edges")
    };
    paths.pts[2 * point + 1] += dy;
    Snapshot::new(parts).expect("valid")
}

/// `snapshot` with edge `edge`'s interior points dropped: the `offsets` column. Legal — the
/// CSR stays non-decreasing and its last entry still counts every point — and the oracle must
/// notice that the edge now has none of the points the convention puts there.
fn unspanned(snapshot: &Snapshot, edge: usize) -> Snapshot {
    let mut parts = snapshot.clone().into_parts();
    let EdgeGeometry::Polyline(paths) = &mut parts.edges else {
        panic!("Polyline edges")
    };
    let first = paths.offsets[edge] as usize;
    let last = paths.offsets[edge + 1] as usize;
    paths.pts.drain(2 * first..2 * last);
    for offset in paths.offsets.iter_mut() {
        *offset = offset.saturating_sub((last - first) as u32);
    }
    Snapshot::new(parts).expect("valid")
}

/// The first interior point of the first edge that has one, or `None`.
fn first_point(snapshot: &Snapshot) -> Option<usize> {
    let p = snapshot.parts();
    let EdgeGeometry::Polyline(paths) = &p.edges else {
        panic!("Polyline edges")
    };
    (paths.offsets[1] > paths.offsets[0]).then_some(paths.offsets[0] as usize)
}

/// The first edge with interior points, or `None`.
fn first_spanned(snapshot: &Snapshot) -> Option<usize> {
    let p = snapshot.parts();
    let EdgeGeometry::Polyline(paths) = &p.edges else {
        panic!("Polyline edges")
    };
    (0..paths.offsets.len() - 1).find(|&e| paths.offsets[e + 1] > paths.offsets[e])
}

/// One snapshot per gate seed, at the gate's own node count: the shapes and the node counts
/// the oracle is actually reached at.
#[test]
fn lanes_matches_its_own_layout_on_the_gate_models() {
    for seed in 0..4 {
        let nodes = graph_core::gate_node_count(seed).min(400);
        let snapshot = super::super::super::pipeline(seed, nodes, "dag.lanes")
            .expect("runs")
            .snapshot;
        assert_eq!(
            super::super::lanes(seed, nodes, &snapshot),
            Ok(()),
            "seed {seed}"
        );
    }
}

/// One control per compared column. Each names its column in the failure, so a control that
/// fired on the wrong one cannot pass for the right one.
#[test]
fn every_compared_column_has_a_control_that_catches_its_perturbation() {
    for seed in 0..2 {
        let nodes = graph_core::gate_node_count(seed).min(400);
        let snapshot = super::super::super::pipeline(seed, nodes, "dag.lanes")
            .expect("runs")
            .snapshot;
        let call = |snap: &Snapshot| super::super::lanes(seed, nodes, snap);
        // The x column.
        let err = call(&moved(&snapshot, 0, (0.5, 0.0))).expect_err("x moved");
        assert!(err.contains("node 0"), "x control, seed {seed}: {err}");
        // The y column — the one `with_x` could never have proved.
        let err = call(&moved(&snapshot, 0, (0.0, 0.5))).expect_err("y moved");
        assert!(err.contains("node 0"), "y control, seed {seed}: {err}");
        // The pts column.
        if let Some(point) = first_point(&snapshot) {
            let err = call(&rebent(&snapshot, point, 0.5)).expect_err("a bend moved");
            assert!(
                err.contains("interior point"),
                "pts control, seed {seed}: {err}"
            );
        }
        // The offsets column.
        if let Some(edge) = first_spanned(&snapshot) {
            let err = call(&unspanned(&snapshot, edge)).expect_err("a span dropped");
            assert!(
                err.contains("interior points"),
                "offsets control, seed {seed}: {err}"
            );
        }
        // The notes column: an extra note on an edge that is not reversed.
        let extra = [Note {
            code: NoteCode::EdgeReversed,
            index: 0,
        }];
        let err = call(&renoted(&snapshot, &extra)).expect_err("a note added");
        assert!(
            err.contains("notes") || err.contains("note "),
            "notes control, seed {seed}: {err}"
        );
    }
}

/// A snapshot of another kind is refused before any arithmetic, so a foreign shape cannot be
/// mistaken for a passing draw.
#[test]
fn a_snapshot_of_another_kind_is_refused_before_any_arithmetic() {
    let grid = super::super::super::pipeline(1, 9, "grid")
        .expect("runs")
        .snapshot;
    assert_eq!(
        super::super::lanes(1, 9, &grid),
        Err("not Point nodes with Polyline edges".into())
    );
}
