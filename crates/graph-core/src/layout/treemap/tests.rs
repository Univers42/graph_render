//! Unit tests for [`super`]. The containment/no-overlap sweep checks every box twice:
//! the exact `f64` values squarify computes, and again after each is independently cast
//! to the contract's `f32` centre/size and its edges reconstructed ([`f32_rect`]). Both
//! passes need a small, separately-justified tolerance — see [`F64_EDGE_EPSILON`] and
//! [`F32_EDGE_EPSILON`] below — and both findings hold at any depth or seed, not only the
//! ones swept here, because neither error compounds with nesting: every box's `f64` edges
//! come from full-precision arithmetic, never from a previously-rounded value one level up.
//!
//! **The `f64` finding.** `tree-degenerate` (an 8-node chain, single child at every level)
//! has a node whose own weight is clamped ([`WEIGHT_EPSILON`]) to a value tiny next to its
//! one child's: `dice`/`slice` compute that child's shared edge as `x0 + value * ((x1 -
//! x0) / value)`, and multiplying by a reciprocal does not always exactly invert the
//! division it came from — the child lands `1` `f64` ULP outside the parent. This is not a
//! bug this port introduced: `treemap/dice.js` and `slice.js` compute the identical
//! `x0 += node.value * k`, so the oracle has the same float behaviour. The error is at
//! most a handful of ULPs per row (one per addition in the row's cumulative sum), so
//! [`F64_EDGE_EPSILON`] needs only a tiny margin over `f64::EPSILON`.
//!
//! **The `f32` finding.** Even the shallow `tree-balanced` fixture (depth 3) has a case:
//! `a` and its child `a1` share an `f64` edge that is bit-identical, but once each box's
//! centre and half-size are rounded to `f32` independently and re-summed, `a1`'s
//! reconstructed edge lands about `3e-8` on the wrong side of `a`'s. Reconstructing an
//! edge from a cast centre and size is a genuinely different computation than casting the
//! edge itself, and the two can disagree by a few `f32` ULPs (~2.4e-7) — bounded by the
//! `[0, 1]` coordinate scale every box uses, not by how small the box is.

use super::*;
use crate::edgekind::{EdgeKind, child_first_from_type};
use crate::index::{Topology, index_model};
use crate::layout::hierarchy::fixture;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;

/// A hierarchy edge `source` -> `target` of wire type `wire` (mirrors `hierarchy`'s own
/// test helper, private to that module).
fn tree(id: &str, source: &str, target: &str, wire: &str) -> EdgeRecord {
    EdgeRecord {
        kind: EdgeKind::Hierarchy,
        child_first: child_first_from_type(Some(wire)),
        label: wire.into(),
        ..edge(id, source, target)
    }
}

fn weighted(id: &str, weight: f64) -> NodeRecord {
    NodeRecord {
        weight,
        ..node(id, "")
    }
}

fn from_fixture(name: &str) -> Topology {
    let (nodes, edges) = fixture::load(name);
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn clamp_weight_only_touches_non_positive_or_non_finite() {
    for bad in [0.0, -5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(clamp_weight(bad), WEIGHT_EPSILON, "{bad}");
    }
    assert_eq!(clamp_weight(0.3), 0.3);
    assert_eq!(
        clamp_weight(1e-7),
        1e-7,
        "small positive is not the epsilon path"
    );
}

#[test]
fn sorted_children_is_descending_value_stable_on_ties() {
    let value = [0.0, 5.0, 9.0, 5.0]; // indices 1, 2, 3 are the candidates below
    assert_eq!(sorted_children(&[1, 2, 3], &value), [2, 1, 3]);
}

/// Root `1.0`, leaves `2.0` and `1.0`: every intermediate value is an exact small
/// integer or a power-of-two fraction, so this is checked against the identical
/// arithmetic (`squarify.js`'s own order), not a hand-typed decimal that could silently
/// mismatch by a rounding a differently-ordered computation would not reproduce.
#[test]
fn a_hand_worked_three_node_tree_matches_the_ported_arithmetic() {
    let nodes = vec![
        weighted("root", 1.0),
        weighted("c1", 2.0),
        weighted("c2", 1.0),
    ];
    let edges = vec![
        tree("r-c1", "root", "c1", "parent_of"),
        tree("r-c2", "root", "c2", "parent_of"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let NodeGeometry::Box { x, y, w, h } = run(&topology).expect("runs").nodes else {
        panic!("box geometry");
    };
    assert_eq!(
        (x[0], y[0], w[0], h[0]),
        (0.5, 0.5, 1.0, 1.0),
        "root is the unit square"
    );

    let k = 1.0_f64 / 3.0_f64;
    let c1_y1 = 2.0_f64 * k;
    let c2_y1 = c1_y1 + 1.0_f64 * k;
    let want = |x0: f64, y0: f64, x1: f64, y1: f64| {
        (
            ((x0 + x1) / 2.0) as f32,
            ((y0 + y1) / 2.0) as f32,
            (x1 - x0) as f32,
            (y1 - y0) as f32,
        )
    };
    assert_eq!(
        (x[1], y[1], w[1], h[1]),
        want(0.0, 0.0, 0.75, c1_y1),
        "c1: root's quarter is the gap"
    );
    assert_eq!(
        (x[2], y[2], w[2], h[2]),
        want(0.0, c1_y1, 0.75, c2_y1),
        "c2 shares c1's column exactly"
    );
}

#[test]
fn every_fixture_is_structural_and_propagates_the_hierarchys_notes() {
    for name in ["tree-balanced", "tree-degenerate", "forest", "cyclic"] {
        let topology = from_fixture(name);
        let hierarchy = Hierarchy::of(&topology).expect("fits");
        let geometry = run(&topology).expect("runs");
        assert_eq!(
            geometry.notes.as_slice(),
            hierarchy.notes(),
            "{name}: notes propagate"
        );
        let NodeGeometry::Box { x, .. } = &geometry.nodes else {
            panic!("box geometry")
        };
        assert_eq!(x.len() as u32, topology.node_count(), "{name}");
        let boxes = compute(&topology, &hierarchy);
        check_invariants(&hierarchy, &boxes, f64_rect, F64_EDGE_EPSILON);
        check_invariants(&hierarchy, &boxes, f32_rect, F32_EDGE_EPSILON);
    }
}

#[test]
fn two_hundred_seeded_topologies_keep_containment_in_f64_and_after_the_f32_cast() {
    for seed in 0..200u32 {
        let (nodes, edges) = seeded_model(seed, 2 + seed * 7 % 120, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("fits");
        let hierarchy = Hierarchy::of(&topology).expect("fits");
        let boxes = compute(&topology, &hierarchy);
        check_invariants(&hierarchy, &boxes, f64_rect, F64_EDGE_EPSILON);
        check_invariants(&hierarchy, &boxes, f32_rect, F32_EDGE_EPSILON);
    }
}

#[test]
fn two_runs_over_the_same_topology_are_byte_equal() {
    let (nodes, edges) = seeded_model(11, 40, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    assert_eq!(run(&topology).expect("runs"), run(&topology).expect("runs"));
}

#[test]
fn no_geometry_is_ever_nan_or_infinite() {
    let mut topologies: Vec<Topology> = ["tree-balanced", "tree-degenerate", "forest", "cyclic"]
        .into_iter()
        .map(from_fixture)
        .collect();
    for seed in 0..32u32 {
        let (nodes, edges) = seeded_model(seed, 3 + seed % 50, REFERENCE_DEGREE);
        topologies.push(index_model(&nodes, &edges).expect("fits"));
    }
    for topology in &topologies {
        let NodeGeometry::Box { x, y, w, h } = run(topology).expect("runs").nodes else {
            panic!("box geometry");
        };
        let finite = x
            .iter()
            .chain(&y)
            .chain(&w)
            .chain(&h)
            .all(|v| v.is_finite());
        assert!(finite, "non-finite geometry");
    }
}

// --- shared invariant machinery ---------------------------------------------------

/// The `f64`-check tolerance (module doc): a `dice`/`slice` row's cumulative sum can miss
/// its outer edge by roughly one `f64` ULP per child in the row; this is generous for any
/// row this crate's own house limits let a snapshot reach.
const F64_EDGE_EPSILON: f64 = 1e-9;

/// The `f32`-check tolerance (module doc): ~40x the ~3e-8 this suite actually observed
/// and ~40x the ~2.4e-7 theoretical bound on independently rounding a centre and a
/// half-size and re-summing them, at the `[0, 1]` coordinate scale every box uses.
const F32_EDGE_EPSILON: f32 = 1e-5;

fn f64_rect(r: Rect) -> (f64, f64, f64, f64) {
    (r.x0, r.y0, r.x1, r.y1)
}

/// Casts `r` to the contract's `f32` centre/size and reconstructs its edges from that —
/// a different rounding path than casting `x0`/`x1` directly, which is exactly what the
/// module doc's `f32` claim needs checked.
fn f32_rect(r: Rect) -> (f32, f32, f32, f32) {
    let cx = ((r.x0 + r.x1) / 2.0) as f32;
    let cy = ((r.y0 + r.y1) / 2.0) as f32;
    let (w, h) = ((r.x1 - r.x0) as f32, (r.y1 - r.y0) as f32);
    (cx - w / 2.0, cy - h / 2.0, cx + w / 2.0, cy + h / 2.0)
}

fn contains<T>(outer: (T, T, T, T), inner: (T, T, T, T), eps: T) -> bool
where
    T: PartialOrd + Copy + std::ops::Add<Output = T>,
{
    outer.0 <= inner.0 + eps
        && inner.2 <= outer.2 + eps
        && outer.1 <= inner.1 + eps
        && inner.3 <= outer.3 + eps
}

fn overlaps<T>(a: (T, T, T, T), b: (T, T, T, T), eps: T) -> bool
where
    T: PartialOrd + Copy + std::ops::Add<Output = T>,
{
    a.0 + eps < b.2 && b.0 + eps < a.2 && a.1 + eps < b.3 && b.1 + eps < a.3
}

/// Every child's box (`rect_of`-projected) sits inside its parent's, and siblings never
/// share interior area, over every row `boxes` holds — the virtual root's included, so a
/// tree with several real roots is checked too even though it is never emitted.
fn check_invariants<T>(
    hierarchy: &Hierarchy,
    boxes: &Boxes,
    rect_of: impl Fn(Rect) -> (T, T, T, T),
    eps: T,
) where
    T: PartialOrd + Copy + std::ops::Add<Output = T> + std::fmt::Debug,
{
    let rows = boxes.0.len() as u32;
    for p in 0..rows {
        let kids = hierarchy.children(p);
        let prect = rect_of(boxes.rect(p));
        for &c in kids {
            let inner = rect_of(boxes.rect(c));
            assert!(
                contains(prect, inner, eps),
                "child {c} escapes parent {p}: outer={prect:?} inner={inner:?}"
            );
        }
        for i in 0..kids.len() {
            for j in (i + 1)..kids.len() {
                let (a, b) = (rect_of(boxes.rect(kids[i])), rect_of(boxes.rect(kids[j])));
                assert!(
                    !overlaps(a, b, eps),
                    "siblings {} {} overlap under {p}",
                    kids[i],
                    kids[j]
                );
            }
        }
    }
}
