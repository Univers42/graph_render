//! The geometry-invariant sweep: containment and no-overlap, checked on the exact `f64`
//! boxes squarify computes and again after each is independently cast to the contract's
//! `f32` centre/size and its edges reconstructed ([`f32_rect`]).
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
//!
//! The two `should_panic` tests at the end are the sweep's **negative control**: a sweep
//! that had silently stopped comparing anything would still be green, so they feed it
//! deliberately broken layouts and require it to reject them.

use super::*;
use crate::index::index_model;
use crate::layout::hierarchy::fixture;
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;

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

#[test]
fn every_fixture_is_structural_and_propagates_the_hierarchys_notes() {
    for name in ["tree-balanced", "tree-degenerate", "forest", "cyclic"] {
        let (nodes, edges) = fixture::load(name);
        let topology = index_model(&nodes, &edges).expect("fits");
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

/// **Negative control (containment).** The sweep is only worth anything if it rejects a
/// bad layout, so this hands it a real `f64` box pushed outside its parent — a state the
/// type under inspection can hold — and requires the panic. Without it, a sweep that had
/// stopped comparing anything would still be green.
#[test]
#[should_panic(expected = "escapes parent")]
fn the_invariant_sweep_rejects_a_child_that_escapes_its_parent() {
    let (nodes, edges) = fixture::load("tree-balanced");
    let topology = index_model(&nodes, &edges).expect("fits");
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let mut boxes = compute(&topology, &hierarchy);
    // Push the root's first child well past the unit square's right edge.
    let child = hierarchy.children(hierarchy.root().expect("a root"))[0];
    let r = boxes.rect(child);
    boxes.set(child, Rect::new(r.x0, r.y0, 1.5, r.y1));
    check_invariants(&hierarchy, &boxes, f64_rect, F64_EDGE_EPSILON);
}

/// **Negative control (overlap).** The sibling-pair half of the sweep, proved live the
/// same way: two siblings are made to share their whole box, which containment alone
/// would not catch.
#[test]
#[should_panic(expected = "overlap under")]
fn the_invariant_sweep_rejects_two_siblings_that_overlap() {
    let (nodes, edges) = fixture::load("tree-balanced");
    let topology = index_model(&nodes, &edges).expect("fits");
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let mut boxes = compute(&topology, &hierarchy);
    let kids = hierarchy.children(hierarchy.root().expect("a root"));
    let (a, b) = (kids[0], kids[1]);
    let r = boxes.rect(a);
    boxes.set(b, r);
    check_invariants(&hierarchy, &boxes, f64_rect, F64_EDGE_EPSILON);
}
