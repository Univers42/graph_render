//! `direct`'s own test, split from `charge.rs` for the house 300-line cap — the same
//! split the module itself already uses (`step.rs`, `tests.rs`).

use super::*;

/// `manyBody.js:77`: `else if (quad.length || l >= distanceMax2) return;` — a leaf
/// reached directly (opening-angle test failed, not internal) contributes nothing
/// once its distance reaches `distanceMax`, exactly like `approx`'s own `dmax2`
/// check. `direct` must not skip this even though it never gets there through
/// `approx`, which takes every cell the opening angle resolves.
#[test]
fn direct_zeroes_a_leaf_beyond_distance_max() {
    let xs = [1000.0];
    let ys = [0.0];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let mut bodies = Vec::new();
    aggregate(&tree, (&xs, &ys), 0.9, &mut bodies);
    assert_eq!(bodies[0].skip, 1, "a single point must build a bare leaf");
    let ctx = Ctx {
        bodies: &bodies,
        tree: &tree,
        x: &xs,
        y: &ys,
        dmin2: 1.0,
        dmax2: 520.0 * 520.0,
        charge: -90.0,
        alpha: 1.0,
        seed: 0,
        tick: 0,
    };
    let q = Query {
        i: 7,
        xi: 0.0,
        yi: 0.0,
    };
    let gap = Gap {
        dx: 1000.0,
        dy: 0.0,
        l: 1000.0 * 1000.0,
    };
    assert_eq!(
        direct(&ctx, &q, &bodies[0], gap),
        (0.0, 0.0),
        "distance 1000 >= distanceMax 520 must zero the direct contribution"
    );
}

/// `open` is `w² / θ²` and the walk reads `open >= l`, so a `NaN` there is read as
/// "approximate this cell as one blob" — the opposite of what `θ = 0` asks for, and a
/// silent `inf/inf` for a cell as wide as `|θ|`.
#[test]
fn an_opening_threshold_is_never_a_nan() {
    for &(w, theta) in &[
        (1.0f64, 0.0f64),
        (0.0, 0.0),
        (0.0, 0.9),
        (1.0, 0.9),
        (1e200, 1e200),
        (f64::INFINITY, f64::INFINITY),
        (1e300, 1e-300),
        (0.0, 1e300),
    ] {
        let theta2 = theta * theta;
        let open = opening_threshold(w, theta, theta2);
        assert!(!open.is_nan(), "w = {w}, theta = {theta}: open is NaN");
    }
}

/// `w * w / θ²` is bit-identical to the squared ratio wherever it was already finite:
/// the threshold is only *computed* differently where the old form overflowed.
#[test]
fn the_opening_threshold_is_the_one_the_old_form_already_produced() {
    for &theta in &[0.3f64, 0.9, 1.5, 0.81, 1e-8] {
        let theta2 = theta * theta;
        for &w in &[0.5, 1.0, 3.25, 1024.0, 1e17, 1e150] {
            assert_eq!(
                opening_threshold(w, theta, theta2).to_bits(),
                (w * w / theta2).to_bits(),
                "w = {w}, theta = {theta} moved"
            );
        }
    }
}

/// The exact-`N²` path: `θ = 0` must open nothing, so two coincident nodes contribute
/// their own terms instead of one blob. `manyBody.js`'s `theta = 0`.
#[test]
fn a_zero_theta_opens_nothing_and_the_walk_stays_exact() {
    let xs = [1.0, 1.0, 1.0, 40.0, 41.0];
    let ys = [2.0, 2.0, 2.0, 3.0, 9.0];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let mut bodies = Vec::new();
    aggregate(&tree, (&xs, &ys), 0.0, &mut bodies);
    for (k, body) in bodies.iter().enumerate() {
        assert!(
            body.open.is_infinite() && body.open > 0.0,
            "cell {k} opens at theta = 0: {}",
            body.open
        );
    }
    let ctx = Ctx {
        bodies: &bodies,
        tree: &tree,
        x: &xs,
        y: &ys,
        dmin2: 1.0,
        dmax2: 1e18,
        charge: -30.0,
        alpha: 1.0,
        seed: 0x9E37_79B9,
        tick: 3,
    };
    for i in 0..xs.len() as u32 {
        let (dx, dy) = node_delta(&ctx, i);
        assert!(dx.is_finite() && dy.is_finite(), "node {i}: ({dx}, {dy})");
    }
}

/// A cell wider than `√f64::MAX` overflows `w * w`; with a `θ` that wide too the quotient
/// is `inf / inf`, i.e. `NaN`, and the whole tree degenerates to one blob per cell.
#[test]
fn a_cell_at_the_extreme_magnitude_still_gets_a_usable_threshold() {
    let xs = [1e200, -1e200, 1e200, 0.5];
    let ys = [1e200, 1e200, -1e200, -0.25];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let mut bodies = Vec::new();
    aggregate(&tree, (&xs, &ys), 1e200, &mut bodies);
    assert!(!bodies.is_empty(), "the tree built");
    for (k, body) in bodies.iter().enumerate() {
        assert!(!body.open.is_nan(), "cell {k}: open is NaN");
    }
    let ctx = Ctx {
        bodies: &bodies,
        tree: &tree,
        x: &xs,
        y: &ys,
        dmin2: 1.0,
        dmax2: f64::INFINITY,
        charge: -30.0,
        alpha: 1.0,
        seed: 0x9E37_79B9,
        tick: 3,
    };
    for i in 0..xs.len() as u32 {
        let (dx, dy) = node_delta(&ctx, i);
        assert!(dx.is_finite() && dy.is_finite(), "node {i}: ({dx}, {dy})");
    }
}

/// The review's unverified item, resolved: `rng.rs` itself is outside this job's paths, so
/// what is pinned here is that `jiggle`'s one word that maps to `0.0` (`h >> 11 == 1 << 52`,
/// one in 2^53 of draws) cannot reach the walk as a zero denominator: `settle` floors a
/// zero `l` to `distanceMin²`, and a fully coincident pair then contributes nothing.
///
/// `rng.rs` is a separate tree that would refuse every other draw, so a witness drawn from
/// the shipped `jiggle` is not available without editing it; this drives `settle` with the
/// exact gap such a draw produces — `dx == dy == 0.0`, `l == 0.0` — which is what
/// `node_delta` cannot be made to hand it any other way.
#[test]
fn a_fully_coincident_pair_at_zero_distance_yields_a_finite_zero_delta() {
    let xs = [1.0, 1.0];
    let ys = [2.0, 2.0];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let mut bodies = Vec::new();
    aggregate(&tree, (&xs, &ys), 0.9, &mut bodies);
    let ctx = Ctx {
        bodies: &bodies,
        tree: &tree,
        x: &xs,
        y: &ys,
        dmin2: 1.0,
        dmax2: 1e18,
        charge: -30.0,
        alpha: 1.0,
        seed: 0,
        tick: 0,
    };
    let q = Query {
        i: 0,
        xi: 1.0,
        yi: 2.0,
    };
    // `dx == dy == 0.0` is what `node_delta`'s gap holds for a pair at one point, and the
    // two guards then replace both axes with `jiggle`.
    let gap = Gap {
        dx: 0.0,
        dy: 0.0,
        l: 0.0,
    };
    let settled = settle(&ctx, &q, 0, gap).expect("distance 0 is inside distanceMax");
    assert!(settled.l > 0.0, "l = {} must leave the divisor", settled.l);
    // And the walk end to end stays finite for every node, coincident pair included.
    for i in 0..xs.len() as u32 {
        let (dx, dy) = node_delta(&ctx, i);
        assert!(dx.is_finite() && dy.is_finite(), "node {i}: ({dx}, {dy})");
    }
}
