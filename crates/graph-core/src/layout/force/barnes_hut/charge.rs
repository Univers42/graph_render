//! The many-body (charge) force via Barnes-Hut approximation. Unlike link/collide this
//! is already a gather — each node reads the (already-built) quadtree and writes only
//! its own velocity, no partner to keep in step with — so it needs none of their Jacobi
//! double-buffering. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/manyBody.js`, simplified: every node carries
//! the same constant charge (`chargeStrength`, `params.rs`), so `accumulate`'s signed
//! `value` collapses to a plain point count times that constant, and a subtree's
//! `(x, y)` is its plain center of mass.

use super::sim::Sim;
use crate::layout::force::quadtree::{Bounds, Quadtree};
use crate::rng::jiggle;

const PASS_X: u32 = 2;
const PASS_Y: u32 = 3;

pub(super) fn apply(sim: &mut Sim) {
    sim.charge_tree.build(&sim.x, &sim.y);
    aggregate(sim);
    for i in 0..sim.x.len() {
        let (dvx, dvy) = node_delta(sim, i as u32);
        sim.vx[i] += dvx;
        sim.vy[i] += dvy;
    }
}

/// Bottom-up mass/center-of-mass per arena node (`manyBody.js`'s `accumulate`).
fn aggregate(sim: &mut Sim) {
    sim.charge_tree.postorder_into(&mut sim.order);
    let len = sim.charge_tree.len() as usize;
    sim.mass.clear();
    sim.mass.resize(len, 0.0);
    sim.comx.clear();
    sim.comx.resize(len, 0.0);
    sim.comy.clear();
    sim.comy.resize(len, 0.0);
    for i in 0..sim.order.len() {
        aggregate_one(sim, sim.order[i]);
    }
}

fn aggregate_one(sim: &mut Sim, node: u32) {
    match sim.charge_tree.children(node) {
        None => {
            let mut points = sim.charge_tree.leaf_points(node);
            let head = points.next().expect("a leaf holds at least one point");
            let count = 1.0 + points.count() as f64;
            sim.mass[node as usize] = count;
            sim.comx[node as usize] = sim.x[head as usize];
            sim.comy[node as usize] = sim.y[head as usize];
        }
        Some(children) => {
            let (mut m, mut cx, mut cy) = (0.0, 0.0, 0.0);
            for c in children.into_iter().flatten() {
                let cm = sim.mass[c as usize];
                m += cm;
                cx += cm * sim.comx[c as usize];
                cy += cm * sim.comy[c as usize];
            }
            sim.mass[node as usize] = m;
            sim.comx[node as usize] = if m > 0.0 { cx / m } else { 0.0 };
            sim.comy[node as usize] = if m > 0.0 { cy / m } else { 0.0 };
        }
    }
}

struct Ctx<'a> {
    mass: &'a [f64],
    comx: &'a [f64],
    comy: &'a [f64],
    x: &'a [f64],
    y: &'a [f64],
    theta2: f64,
    dmin2: f64,
    dmax2: f64,
    charge: f64,
    alpha: f64,
    seed: u32,
    tick: u32,
}

struct Query {
    i: u32,
    xi: f64,
    yi: f64,
}

fn node_delta(sim: &mut Sim, i: u32) -> (f64, f64) {
    let xi = sim.x[i as usize];
    let yi = sim.y[i as usize];
    let ctx = Ctx {
        mass: &sim.mass,
        comx: &sim.comx,
        comy: &sim.comy,
        x: &sim.x,
        y: &sim.y,
        theta2: sim.params.theta * sim.params.theta,
        dmin2: sim.params.distance_min * sim.params.distance_min,
        dmax2: sim.params.distance_max * sim.params.distance_max,
        charge: sim.params.charge_strength,
        alpha: sim.alpha,
        seed: sim.seed,
        tick: sim.tick_no,
    };
    let q = Query { i, xi, yi };
    let mut out = (0.0, 0.0);
    let mut frame = Frame {
        ctx: &ctx,
        q: &q,
        out: &mut out,
    };
    sim.charge_tree
        .visit(|tree, node, bounds| step(tree, node, bounds, &mut frame));
    out
}

/// `ctx`, `q` and the accumulator bundled so [`step`] stays under the 4-param cap
/// (`refactor-rust.md`) despite the quadtree visit callback's own fixed 3 arguments.
struct Frame<'a> {
    ctx: &'a Ctx<'a>,
    q: &'a Query,
    out: &'a mut (f64, f64),
}

/// One quadtree node reached while querying node `q.i`: applies the Barnes-Hut
/// approximation when the opening angle allows it, else falls through to `direct`
/// (`manyBody.js`'s own `apply`).
fn step(tree: &Quadtree, node: u32, bounds: Bounds, frame: &mut Frame) -> bool {
    if let Some((prune, delta)) = approx(frame.ctx, node, bounds, frame.q) {
        frame.out.0 += delta.0;
        frame.out.1 += delta.1;
        return prune;
    }
    if tree.children(node).is_some() {
        return false;
    }
    let delta = direct(frame.ctx, tree, node, frame.q);
    frame.out.0 += delta.0;
    frame.out.1 += delta.1;
    false
}

/// `Some((true, delta))` when the opening-angle test resolves this node as one blob
/// (always pruned, `delta` zero past `distanceMax`); `None` when it is too close or too
/// coarse and the caller must recurse or fall through to a leaf's exact points.
fn approx(ctx: &Ctx, node: u32, bounds: Bounds, q: &Query) -> Option<(bool, (f64, f64))> {
    let m = ctx.mass[node as usize];
    if m == 0.0 {
        return Some((true, (0.0, 0.0)));
    }
    let mut dx = ctx.comx[node as usize] - q.xi;
    let mut dy = ctx.comy[node as usize] - q.yi;
    let w = bounds.x1 - bounds.x0;
    let mut l = dx * dx + dy * dy;
    if w * w / ctx.theta2 >= l {
        return None;
    }
    if l >= ctx.dmax2 {
        return Some((true, (0.0, 0.0)));
    }
    if dx == 0.0 {
        dx = jiggle(ctx.seed, ctx.tick, PASS_X, (q.i, node));
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(ctx.seed, ctx.tick, PASS_Y, (q.i, node));
        l += dy * dy;
    }
    if l < ctx.dmin2 {
        l = libm::sqrt(ctx.dmin2 * l);
    }
    let f = m * ctx.charge * ctx.alpha / l;
    Some((true, (dx * f, dy * f)))
}

/// A leaf too close (or too coarse) to approximate: every distinct chained point
/// contributes its own charge directly (`manyBody.js`'s coincidence-chain loop), unless
/// `distanceMax` has already been reached, in which case it contributes nothing — the
/// same cutoff `approx` applies to the Barnes-Hut-approximable case (`manyBody.js:77`:
/// `else if (quad.length || l >= distanceMax2) return;`).
fn direct(ctx: &Ctx, tree: &Quadtree, node: u32, q: &Query) -> (f64, f64) {
    let Some(head) = tree.leaf_points(node).next() else {
        return (0.0, 0.0);
    };
    let mut dx = ctx.x[head as usize] - q.xi;
    let mut dy = ctx.y[head as usize] - q.yi;
    let mut l = dx * dx + dy * dy;
    if l >= ctx.dmax2 {
        return (0.0, 0.0);
    }
    if dx == 0.0 {
        dx = jiggle(ctx.seed, ctx.tick, PASS_X, (q.i, head));
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(ctx.seed, ctx.tick, PASS_Y, (q.i, head));
        l += dy * dy;
    }
    if l < ctx.dmin2 {
        l = libm::sqrt(ctx.dmin2 * l);
    }
    let (mut dvx, mut dvy) = (0.0, 0.0);
    for p in tree.leaf_points(node) {
        if p == q.i {
            continue;
        }
        let f = ctx.charge * ctx.alpha / l;
        dvx += dx * f;
        dvy += dy * f;
    }
    (dvx, dvy)
}

#[cfg(test)]
mod tests {
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
}
