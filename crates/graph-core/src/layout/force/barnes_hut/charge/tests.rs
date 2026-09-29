//! `direct`'s own test, split from `charge.rs` for the house 300-line cap — the same
//! split the module itself already uses (`step.rs`, `tests.rs`).

use super::*;

fn ctx_for_direct<'a>(xs: &'a [f64], ys: &'a [f64]) -> Ctx<'a> {
    Ctx {
        mass: &[],
        comx: &[],
        comy: &[],
        x: xs,
        y: ys,
        theta2: 0.81,
        dmin2: 1.0,
        dmax2: 520.0 * 520.0,
        charge: -90.0,
        alpha: 1.0,
        seed: 0,
        tick: 0,
    }
}

/// `manyBody.js:77`: `else if (quad.length || l >= distanceMax2) return;` — a leaf
/// reached directly (opening-angle test failed, not internal) contributes nothing
/// once its distance reaches `distanceMax`, exactly like `approx`'s own `dmax2`
/// check. `direct` must not skip this even though it never gets there through
/// `approx` (which already returns `Some` and short-circuits `step` for that case).
#[test]
fn direct_zeroes_a_leaf_beyond_distance_max() {
    let xs = [1000.0];
    let ys = [0.0];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let root = 0;
    assert!(
        tree.children(root).is_none(),
        "a single point must build a bare leaf"
    );
    let ctx = ctx_for_direct(&xs, &ys);
    let q = Query {
        i: 7,
        xi: 0.0,
        yi: 0.0,
    };
    let (dvx, dvy) = direct(&ctx, &tree, root, &q);
    assert_eq!(
        (dvx, dvy),
        (0.0, 0.0),
        "distance 1000 >= distanceMax 520 must zero the direct contribution"
    );
}
