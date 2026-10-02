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
    aggregate(&tree, (&xs, &ys), 0.81, &mut bodies);
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
