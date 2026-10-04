//! The 3D many-body walk's suite. Split from `../tests.rs` for the house 300-line cap.
//!
//! The load-bearing test is `at_theta_zero_the_tree_and_a_pairwise_scan_agree`, and its
//! negative control is `at_theta_zero_the_agreement_is_not_vacuous`: the same comparison at
//! `θ = 0.9` must **fail**, or the first test would also pass for a walk that returns
//! zeros or that never descends.
//!
//! Agreement is measured against the **condition number** of the sum — the total magnitude
//! of the terms the pairwise scan added, not the size of its answer. A many-body delta is a
//! near-cancelling sum (a node inside a cluster is pushed almost equally in both
//! directions), so comparing two such sums relative to the answer would demand more than
//! `f64` can deliver and the row would be unfalsifiable in the wrong direction.

use super::super::charge::{self, threshold, Body, Gap, Query, Terms, Walk};
use super::super::{Octree, Points3};
use crate::layout::force::{ForceParams, LiveParams};

/// A point set with no two points sharing any coordinate, so no pair takes the jiggle
/// branch and the pairwise reference below needs none of `settle`'s heuristics.
fn separated(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let xs = (0..n).map(|i| f64::from(i) + 1.0).collect();
    let ys = (0..n).map(|i| 0.37 * f64::from(i) + 0.11).collect();
    let zs = (0..n).map(|i| 0.11 * f64::from(i) - 0.7).collect();
    (xs, ys, zs)
}

/// A point set that *does* share axes — a grid — which is where `settle`'s zero-axis jiggle
/// and its `distanceMin` floor are the whole story. With `coincident`, three further points
/// land on exactly the same coordinate, which is the one place the tree and the pairwise
/// reference below are **not** built the same way (see the chain test).
fn grid(side: u32, coincident: bool) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut cols = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..side {
        for j in 0..side {
            for k in 0..side {
                cols.0.push(f64::from(i) * 3.0);
                cols.1.push(f64::from(j) * 3.0);
                cols.2.push(f64::from(k) * 3.0);
            }
        }
    }
    if coincident {
        for _ in 0..3 {
            cols.0.push(11.0);
            cols.1.push(13.0);
            cols.2.push(17.0);
        }
    }
    cols
}

/// The frozen terms with `alpha = 1.0`, seed and tick both zero — the numbers every test
/// here compares at.
fn terms() -> Terms {
    Terms::of(&LiveParams::from(ForceParams::default()), (1.0, 0, 0))
}

/// Every node's walk delta over `cols` at `theta`, prepared the way a tick prepares it:
/// build, then aggregate in reverse preorder, then one query per point in point order.
fn tree_deltas(cols: (Vec<f64>, Vec<f64>, Vec<f64>), theta: f64) -> Vec<(f64, f64, f64)> {
    let (xs, ys, zs) = cols;
    let pts = Points3 {
        xs: &xs,
        ys: &ys,
        zs: &zs,
    };
    let mut tree = Octree::default();
    tree.build(pts);
    let mut bodies = Vec::new();
    charge::aggregate(&tree, pts, theta, &mut bodies);
    let walk = Walk::new(&bodies, &tree, pts, terms());
    (0..pts.len() as u32).map(|i| walk.node(i)).collect()
}

/// One reference answer: the delta, and the total magnitude of the terms that produced it.
struct Ref {
    delta: (f64, f64, f64),
    work: f64,
}

/// The pairwise reference: every other point, ascending, through the *same* `settle` tail
/// the tree uses, so the comparison isolates the traversal and not the heuristics.
fn pairwise(cols: &(Vec<f64>, Vec<f64>, Vec<f64>), i: u32) -> Ref {
    let (xs, ys, zs) = cols;
    let pts = Points3 { xs, ys, zs };
    let terms = terms();
    let q = Query {
        i,
        p: pts.at(i),
    };
    let mut delta = (0.0, 0.0, 0.0);
    let mut work = 0.0;
    for j in 0..pts.len() as u32 {
        if j == i {
            continue;
        }
        let (xj, yj, zj) = pts.at(j);
        let (dx, dy, dz) = (xj - q.p.0, yj - q.p.1, zj - q.p.2);
        let gap = Gap {
            dx,
            dy,
            dz,
            l: dx * dx + dy * dy + dz * dz,
        };
        let Some(gap) = charge::settle(&terms, &q, j, gap) else {
            continue;
        };
        let f = terms.charge * terms.alpha / gap.l;
        let term = (gap.dx * f, gap.dy * f, gap.dz * f);
        work += term.0.abs() + term.1.abs() + term.2.abs();
        delta.0 += term.0;
        delta.1 += term.1;
        delta.2 += term.2;
    }
    Ref { delta, work }
}

/// The disagreement between two deltas, in units of the sum's own condition number.
fn residual(got: (f64, f64, f64), want: &Ref) -> f64 {
    let worst = (got.0 - want.delta.0)
        .abs()
        .max((got.1 - want.delta.1).abs())
        .max((got.2 - want.delta.2).abs());
    if want.work == 0.0 {
        return if worst == 0.0 { 0.0 } else { f64::INFINITY };
    }
    worst / want.work
}

#[test]
fn at_theta_zero_the_tree_and_a_pairwise_scan_agree() {
    for cols in [separated(24), grid(3, false)] {
        let tree = tree_deltas(cols.clone(), 0.0);
        for (i, got) in tree.iter().enumerate() {
            let want = pairwise(&cols, i as u32);
            assert!(
                residual(*got, &want) <= 1e-12,
                "node {i}: tree {got:?} against pairwise {:?} on a {n}-point set",
                want.delta,
                n = cols.0.len()
            );
        }
    }
}

/// The negative control for the row above: at the shipped `θ` the two **disagree**, so the
/// agreement at `θ = 0` is the traversal being exact and not the tolerance being loose.
#[test]
fn at_theta_zero_the_agreement_is_not_vacuous() {
    let cols = grid(4, false);
    let tree = tree_deltas(cols.clone(), 0.9);
    for (i, got) in tree.iter().enumerate() {
        let want = pairwise(&cols, i as u32);
        assert!(
            residual(*got, &want) > 1e-6,
            "node {i} at theta 0.9 agrees to {got:?} / {:?}, so the theta-0 row proves nothing",
            want.delta
        );
    }
}

/// The one place `θ = 0` is **not** a pairwise scan, stated rather than smoothed over: a
/// leaf's coincident chain shares **one** settled gap, because `direct` keys `settle` on the
/// chain's head and then applies that gap to every member. The pairwise reference keys on
/// each partner, so for a chain the two give different jiggled axes — the same choice the 2D
/// walk makes, and the reason this row asserts a difference rather than hiding one.
///
/// What is pinned is that the difference is a real one: with `charge = -90` a chain charged
/// at the head's jiggled separation is a far weaker push than the pairwise scan's.
#[test]
fn a_coincident_chain_shares_one_settled_gap_so_it_is_not_the_pairwise_sum() {
    let cols = grid(3, true);
    let tree = tree_deltas(cols.clone(), 0.0);
    let mut checked = 0;
    for (i, got) in tree.iter().enumerate().skip(27) {
        let want = pairwise(&cols, i as u32);
        assert!(
            residual(*got, &want) > 1e-6,
            "coincident node {i}: tree {got:?} agrees with pairwise {:?}, so the chain is not sharing a gap",
            want.delta
        );
        checked += 1;
    }
    assert_eq!(checked, 3, "all three coincident points were compared");
}

/// `θ = 0` opens nothing: every cell's threshold is infinite, so the walk descends to the
/// leaves and sums the exact pairwise terms. This is the property the rows above rest on,
/// stated on its own.
#[test]
fn theta_zero_makes_every_cell_unapproximable() {
    assert_eq!(threshold::opening_threshold(1.0, 0.0, 0.0), f64::INFINITY);
    assert_eq!(threshold::opening_threshold(64.0, 0.0, 0.0), f64::INFINITY);
}

/// `θ > 0` makes it the cube's edge over `θ`, squared — the same quantity the quadtree uses
/// for its square's side, so one `θ` means the same thing in three dimensions as in two.
#[test]
fn the_opening_threshold_is_the_cube_edge_over_theta_squared() {
    let (w, theta) = (8.0, 0.5);
    assert_eq!(
        threshold::opening_threshold(w, theta, theta * theta),
        w * w / (theta * theta)
    );
    // A `NaN` ratio opens nothing rather than approximating, as in 2D.
    assert_eq!(
        threshold::opening_threshold(0.0, 0.0, 0.0),
        f64::INFINITY
    );
    assert_eq!(
        threshold::opening_threshold(f64::INFINITY, 2.0, 4.0),
        f64::INFINITY
    );
}

/// The aggregate is a mass-weighted centre over three axes, in slot order, and it is what
/// an approximated cell contributes.
#[test]
fn the_centre_of_mass_is_mass_weighted_over_three_axes() {
    let bodies = [
        Body {
            count: 1,
            skip: 1,
            start: 0,
            ..Body::default()
        },
        Body {
            comx: 3.0,
            comy: 6.0,
            comz: 9.0,
            count: 3,
            skip: 2,
            start: 1,
            ..Body::default()
        },
    ];
    assert_eq!(
        threshold::centre(&bodies, 0, 2),
        (2.25, 4.5, 6.75),
        "(1 * 0 + 3 * 3) / 4 and so on"
    );
}