//! The overlap invariant, the determinism properties, and the refusals. The brute-force
//! all-pairs scan every property here leans on is a **test instrument, not product**: it is
//! `O(n^2)`, so [`super::run`] never calls it and nothing in `crates/` does either. It is
//! what makes the invariant exact rather than "usually".

mod degradation;
mod refusals;
mod sweep_tests;

use super::*;
use crate::index::Topology;
use crate::records::build::{edge, node};
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Every node at `(0, 0)`, `n` of them: the degenerate input. Discs stacked on one point is
/// the case a sweep has to break symmetry on or it converges to nothing.
fn stacked(n: usize, r: f32) -> Geometry {
    Geometry::planar(
        NodeGeometry::Circle {
            x: vec![0.0; n],
            y: vec![0.0; n],
            r: vec![r; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// Every node on one line at `(i, 0)`: the input where a sweep has to break symmetry along
/// one axis only.
fn in_line(n: usize, r: f32) -> Geometry {
    Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| i as f32).collect(),
            y: vec![0.0; n],
            r: vec![r; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// A path graph over `n` nodes, so [`run`] is given a topology it can check against.
fn path_topology(n: usize) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    crate::index::index_model(&nodes, &edges).expect("fits")
}

/// **The invariant**, as an exact property: after the pass no two nodes overlap by more than
/// the tolerance. One brute-force pass over every pair, no sampling.
fn assert_separated(geometry: &Geometry, r: f32, margin: f32, where_: &str) {
    let worst = worst_overlap(geometry, r, margin);
    assert!(
        worst <= TOLERANCE,
        "{where_}: worst overlap {worst} exceeds the tolerance {TOLERANCE}"
    );
}

/// The deepest single penetration between any two discs, in layout units; 0 when none. The
/// `O(n²)` scan behind the invariant, and **a test instrument only** — nothing in
/// `crates/` calls it, which is why it can be exact where the pass is `O(n · k)`.
fn worst_overlap(geometry: &Geometry, r: f32, margin: f32) -> f32 {
    let NodeGeometry::Circle { x, y, .. } = &geometry.nodes else {
        panic!("expected circles");
    };
    let mut worst = 0.0f32;
    for i in 0..x.len() {
        for j in i + 1..x.len() {
            let d = libm::sqrtf((x[i] - x[j]) * (x[i] - x[j]) + (y[i] - y[j]) * (y[i] - y[j]));
            worst = worst.max(r + r + margin - d);
        }
    }
    worst
}

#[test]
fn stacked_discs_are_separated() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &stacked(n, 1.0)).expect("runs");
    assert_separated(&bundled.geometry, 1.0, 0.0, "stacked");
}

#[test]
fn line_of_discs_is_separated() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &in_line(n, 1.0)).expect("runs");
    assert_separated(&bundled.geometry, 1.0, 0.0, "line");
}

#[test]
fn the_margin_is_honoured() {
    let n = 200;
    let topology = path_topology(n);
    let params = SeparateParams {
        margin: 2.0,
        ..SeparateParams::default()
    };
    let bundled = separate(&topology, &stacked(n, 1.0), &params).expect("runs");
    assert_separated(&bundled.geometry, 1.0, 2.0, "stacked + margin");
}

/// **D1: bit-identical across runs.** The same input run twice is the same bits, and the
/// two are compared as `f32` rather than "close", because "close" is what a
/// non-deterministic reduction hides behind.
#[test]
fn the_pass_is_deterministic() {
    let topology = path_topology(300);
    let input = in_line(300, 1.0);
    let first = run(&topology, &input).expect("runs");
    let second = run(&topology, &input).expect("runs");
    assert_eq!(first.geometry.nodes, second.geometry.nodes);
    assert_eq!(first.pairs, second.pairs);
    assert_eq!(first.unbundled, second.unbundled);
}

/// **A `Point` is a no-op at the default**, because a point has no size and the pass will
/// not invent one. This is the claim the decision doc's §2 rests on.
#[test]
fn a_point_layout_is_untouched_at_the_default_radius() {
    let n = 100;
    let topology = path_topology(n);
    let input = Geometry::planar(
        NodeGeometry::Point {
            x: vec![0.0; n],
            y: vec![0.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    );
    let bundled = run(&topology, &input).expect("runs");
    assert_eq!(bundled.geometry.nodes, input.nodes);
    assert_eq!(bundled.pairs, 0);
}

/// A `Point` layout with a caller-supplied radius *is* separated, which is what makes the
/// no-op above a choice rather than a dead end.
#[test]
fn a_point_radius_separates() {
    let n = 100;
    let topology = path_topology(n);
    let params = SeparateParams {
        point_radius: 1.0,
        ..SeparateParams::default()
    };
    let bundled = separate(
        &topology,
        &Geometry::planar(
            NodeGeometry::Point {
                x: vec![0.0; n],
                y: vec![0.0; n],
            },
            EdgeGeometry::Line,
            Vec::new(),
        ),
        &params,
    )
    .expect("runs");
    let NodeGeometry::Point { x, y } = &bundled.geometry.nodes else {
        panic!("points stay points");
    };
    let mut worst = 0.0f32;
    for i in 0..n {
        for j in i + 1..n {
            let d = libm::sqrtf((x[i] - x[j]) * (x[i] - x[j]) + (y[i] - y[j]) * (y[i] - y[j]));
            worst = worst.max(2.0 - d);
        }
    }
    assert!(worst <= 1e-3, "point discs overlap by {worst}");
}

/// A `Box` is separated by its circumscribed radius, so two boxes that share a bounding disc
/// are pushed apart — over-separating, which §2 of the decision doc names as the safe
/// direction.
#[test]
fn a_box_layout_is_separated_by_its_circumscribed_radius() {
    let n = 200;
    let topology = path_topology(n);
    let input = Geometry::planar(
        NodeGeometry::Box {
            x: vec![0.0; n],
            y: vec![0.0; n],
            w: vec![2.0; n],
            h: vec![2.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    );
    let bundled = run(&topology, &input).expect("runs");
    let NodeGeometry::Box { x, y, .. } = &bundled.geometry.nodes else {
        panic!("boxes stay boxes");
    };
    // half-diagonal of a 2x2 box
    let r = libm::sqrtf(2.0);
    let mut worst = 0.0f32;
    for i in 0..n {
        for j in i + 1..n {
            let d = libm::sqrtf((x[i] - x[j]) * (x[i] - x[j]) + (y[i] - y[j]) * (y[i] - y[j]));
            worst = worst.max(2.0 * r - d);
        }
    }
    assert!(worst <= 1e-3, "box discs overlap by {worst}");
}
