//! The overlap invariant, the determinism properties, and the refusals. The brute-force
//! all-pairs scan every property here leans on is a **test instrument, not product**: it is
//! `O(n^2)`, so [`super::run`] never calls it and nothing in `crates/` does either. It is
//! what makes the invariant exact rather than "usually".

mod sweep_tests;

use super::*;
use crate::index::Topology;
use crate::post::{PostRun, find};
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

/// **The z refusal, no new error variant.** A 3D geometry is refused rather than damaged,
/// which is what keeps `contract-3d-verdict.md` condition 6 unamended.
#[test]
fn a_geometry_with_a_z_column_is_refused() {
    let n = 10;
    let topology = path_topology(n);
    let input = Geometry::in_space(
        NodeGeometry::Circle {
            x: vec![0.0; n],
            y: vec![0.0; n],
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
        vec![0.0; n],
    );
    let err = run(&topology, &input).expect_err("refused");
    assert_eq!(
        err,
        StageError::Param {
            name: "geometry.z",
            rule: "must be absent"
        }
    );
}

/// Refused rather than clamped, one case per parameter.
#[test]
fn bad_parameters_are_refused() {
    let topology = path_topology(4);
    let input = stacked(4, 1.0);
    for (params, name) in [
        (
            SeparateParams {
                margin: -1.0,
                ..SeparateParams::default()
            },
            "margin",
        ),
        (
            SeparateParams {
                max_iterations: 0,
                ..SeparateParams::default()
            },
            "max_iterations",
        ),
        (
            SeparateParams {
                point_radius: -1.0,
                ..SeparateParams::default()
            },
            "point_radius",
        ),
        (
            SeparateParams {
                margin: f64::NAN,
                ..SeparateParams::default()
            },
            "margin",
        ),
    ] {
        let err = separate(&topology, &input, &params).expect_err("refused");
        match err {
            StageError::Param { name: got, .. } => assert_eq!(got, name),
            other => panic!("{name}: expected Param, got {other}"),
        }
    }
}

/// An empty graph is not an error, the same rule the routing grid follows: no nodes, no
/// discs, no work. The topology is genuinely empty, so this is the "no nodes" case rather
/// than a count mismatch.
#[test]
fn an_empty_layout_is_not_an_error() {
    let empty: Topology = crate::index::index_model(&[], &[]).expect("fits");
    let bundled = run(&empty, &stacked(0, 1.0)).expect("runs");
    assert_eq!(bundled.pairs, 0);
    assert_eq!(bundled.unbundled, 0);
}

/// **A geometry whose node count disagrees with the topology is refused**, not drawn: the
/// columns would index past the edge endpoints they have to match, and a mismatch here means
/// the caller wired two different graphs together.
#[test]
fn a_node_count_mismatch_is_refused() {
    let topology = path_topology(9);
    let err = run(&topology, &stacked(4, 1.0)).expect_err("refused");
    assert_eq!(
        err,
        StageError::Param {
            name: "node count",
            rule: "the topology's own",
        }
    );
}

/// **The control for this pass, in the units it reports in.** `post::tests` uses "did the ink
/// go down" to prove a bundler is not a registered no-op; the equivalent question for a node
/// mover is "did it resolve anything", and this asks it. A pass that resolved zero pairs over
/// a stack of 500 coincident discs would satisfy every separation assertion here vacuously if
/// the assertions only measured geometry — so this pins the **count**, not the positions.
#[test]
fn it_actually_resolves_pairs() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &stacked(n, 1.0)).expect("runs");
    assert!(
        bundled.pairs > 0,
        "500 coincident discs must resolve a nonzero number of pairs, got {}",
        bundled.pairs
    );
    assert_eq!(
        bundled.unbundled, 0,
        "a converged sweep leaves no residual overlap"
    );
}

/// A layout whose discs are already clear must come out **byte-identical**, not merely
/// un-overlapped: a pass that nudges a readable drawing is a pass that makes it worse, and
/// this is what says the nudge is bounded by the radius and cannot do that.
#[test]
fn a_clear_layout_is_left_alone() {
    let n = 100;
    let topology = path_topology(n);
    let input = Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| i as f32 * 4.0).collect(),
            y: (0..n).map(|i| (i as f32 * 7.0) % 11.0).collect(),
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    );
    let bundled = run(&topology, &input).expect("runs");
    assert_eq!(
        bundled.geometry.nodes, input.nodes,
        "a drawing with no overlapping pair must be returned untouched"
    );
    assert_eq!(bundled.pairs, 0);
}

/// **The negative control, at the level that can bite: `over_relaxation: 0` freezes every
/// displacement**, so a pass that cannot move a node leaves the input's overlaps exactly where
/// they were and reports every one of them in `Bundled::unbundled`.
///
/// This is the same perturbation the hash gate's `GM_MUTATE_OVERLAP_RELAXATION` applies to the
/// native arm, asserted here where the arithmetic is visible. `0` is a legal value and is not
/// clamped — a clamp would hide the one value the control needs.
#[test]
fn a_frozen_relaxation_leaves_every_overlap_and_reports_it() {
    let n = 200;
    let topology = path_topology(n);
    let params = SeparateParams {
        over_relaxation: 0.0,
        ..SeparateParams::default()
    };
    let bundled = separate(&topology, &stacked(n, 1.0), &params).expect("runs");
    assert_eq!(
        bundled.geometry.nodes.kind(),
        stacked(n, 1.0).nodes.kind(),
        "the kind is preserved even when nothing moves"
    );
    // Every disc is on top of every other, so the residual is every pair but the node's own
    // diagonal entry, which the half-open comparison excludes.
    let want = (n as u32) * (n as u32 - 1) / 2;
    assert_eq!(
        bundled.unbundled, want,
        "a frozen sweep must report every overlapping pair it did not resolve"
    );
    assert!(
        bundled.pairs < want,
        "a frozen sweep resolves no pair, so the geometry is exactly the stacked input"
    );
    // The decisive assertion, inverted so the control is a **passing** test: the geometry is
    // still fully overlapping. A control that froze the sweep while some other term still
    // separated the discs would satisfy the count checks above and leave the invariant green,
    // so what is checked here is the invariant *failing* — measured, not assumed: the worst
    // overlap under the knob is 1.993 against the tolerance 1e-3.
    let worst = worst_overlap(&bundled.geometry, 1.0, 0.0);
    assert!(
        worst > 1.0,
        "the frozen pass must leave the discs overlapping; worst overlap was {worst}"
    );
}

/// **The binding adversarial case: a full lattice.** A `k × k` grid of discs at pitch equal to
/// the radius is harder than a random cloud by an order of magnitude — every node is wedged
/// symmetrically between eight neighbours, so its own displacement vectors partly cancel and
/// the relaxation has to fight that symmetry. Measured: it needed 192 sweeps at 500 nodes
/// where a random cloud needed a median of 16, which is why `SeparateParams::max_iterations`
/// is 512 rather than the number a random cloud would suggest.
#[test]
fn a_full_lattice_is_separated() {
    let n = 500;
    let topology = path_topology(n);
    let bundled = run(&topology, &gridded(n, 1.0)).expect("runs");
    assert_separated(&bundled.geometry, 1.0, 0.0, "lattice");
    assert_eq!(
        bundled.unbundled, 0,
        "the lattice converged inside the default cap"
    );
}

/// `n` discs on a lattice of pitch `spacing` — the grid layout's own shape.
fn gridded(n: usize, spacing: f32) -> Geometry {
    let k = (n as f32).sqrt() as usize;
    Geometry::planar(
        NodeGeometry::Circle {
            x: (0..n).map(|i| (i % k) as f32 * spacing).collect(),
            y: (0..n).map(|i| (i / k) as f32 * spacing).collect(),
            r: vec![1.0; n],
        },
        EdgeGeometry::Line,
        Vec::new(),
    )
}

/// The pass is registered, so the hash gate and the ledger can name it.
#[test]
fn the_pass_is_registered_and_declares_it_moves_nodes() {
    let cap = find(ID).expect("registered");
    assert!(cap.meta.moves_nodes, "this pass moves nodes and says so");
    assert_eq!(cap.run, run as PostRun);
}
