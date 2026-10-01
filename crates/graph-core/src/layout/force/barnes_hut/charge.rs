//! The many-body (charge) force via Barnes-Hut approximation. Unlike link/collide this
//! is already a gather — each node reads the (already-built) quadtree and writes only
//! its own velocity, no partner to keep in step with — so it needs none of their Jacobi
//! double-buffering. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/manyBody.js`, simplified: every node carries
//! the same constant charge (`chargeStrength`, `params.rs`), so `accumulate`'s signed
//! `value` collapses to a plain point count times that constant, and a subtree's
//! `(x, y)` is its plain center of mass.

use super::sim::Sim;
use crate::exec::Runner;
use crate::layout::force::quadtree::{Bounds, Quadtree};
use crate::rng::jiggle;

const PASS_X: u32 = 2;
const PASS_Y: u32 = 3;

/// The many-body pass, with its per-node gather divided by `runner` over `workers` workers.
///
/// `charge::apply_with(&Serial, 1, ..)` is the serial pass, and
/// `BarnesHut::run_with(&Serial, 1)` is the stage — one name, not three, so there is no
/// second spelling of the same pass to drift.
///
/// The merge is a straight loop over `deltas` in ascending node index, which is the one
/// place the division could go wrong and the reason it is written as a loop rather than
/// left to the runner: each node adds its *own* delta to its *own* velocity, and
/// `i += delta[i]` cannot pick up a neighbour's term no matter how the gather was sliced.
///
/// `split` is this pass's own slice of the negative control (`Split::CHARGE`): it makes the
/// merge read the **next** node's delta as well, the shape a wrong partition of the
/// outputs would take. It is a parameter rather than a `cfg` or an environment read so that
/// (a) the mutated path is a compiled-in branch a test can call directly, and (b) graph-core
/// reads no clock, no environment and no hardware — the host supplies even the mutation.
pub(super) fn apply_with(
    sim: &mut Sim,
    runner: &impl Runner,
    workers: u32,
    deltas: &mut Vec<(f64, f64)>,
    split: bool,
) {
    prepare(sim);
    runner.run(&super::step::Pass::of(&*sim), workers, deltas);
    merge(sim, deltas, split);
}

/// `vx[i] += deltas[i]`, in ascending node index — and, under the control, the next node's
/// delta too. Ascending index and one node's own delta, which is why the control has to
/// *steal* a neighbour's term to move anything.
fn merge(sim: &mut Sim, deltas: &[(f64, f64)], split: bool) {
    for (i, (dvx, dvy)) in deltas.iter().enumerate() {
        let stolen = if split {
            deltas.get(i + 1).copied().unwrap_or((0.0, 0.0))
        } else {
            (0.0, 0.0)
        };
        sim.vx[i] += dvx + stolen.0;
        sim.vy[i] += dvy + stolen.1;
    }
}

/// The single-threaded prologue every many-body pass shares: build the quadtree over this
/// tick's positions, then aggregate masses and centres bottom-up.
///
/// Split out of [`apply`] because the range kernel ([`super::step::Pass`]) needs the same
/// two steps in front of it and must not have its own copy — a second tree build or a
/// second aggregate order would be a silent divergence from the serial pass, and the
/// equality test would then be comparing two programs rather than two schedules.
pub(super) fn prepare(sim: &mut Sim) {
    sim.charge_tree.build(&sim.x, &sim.y);
    aggregate(sim);
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

pub(super) struct Ctx<'a> {
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

pub(super) struct Query {
    i: u32,
    xi: f64,
    yi: f64,
}

/// Node `i`'s own many-body delta, over an already-prepared [`Sim`], into a caller's
/// reused walk stack.
///
/// `&self` and a borrowed stack, which is what lets the range kernel
/// ([`super::step::Pass`]) hold one `&Sim` and have every worker walk the same tree at
/// once: the walk is iterative precisely so its buffer can be borrowed rather than owned
/// (`quadtree.rs`'s `visit_in`), and each worker brings its own.
///
/// The stack is a parameter rather than a local because the serial pass runs this once
/// per node over `TICKS × n` walks: a local `Vec` here would be an allocation per node,
/// which is the one thing `dsa-and-memory.md` forbids in a per-tick loop. One buffer per
/// pass — the serial loop's, or one per range in a threaded run — is the right count.
pub(super) fn node_delta_with(sim: &Sim, i: u32, stack: &mut Vec<(u32, Bounds)>) -> (f64, f64) {
    let ctx = Ctx {
        mass: &sim.mass,
        comx: &sim.comx,
        comy: &sim.comy,
        x: &sim.x,
        y: &sim.y,
        theta2: sim.params.theta * sim.params.theta,
        dmin2: sim.params.distance_min * sim.params.distance_min,
        dmax2: sim.params.distance_max * sim.params.distance_max,
        charge: sim.params.charge,
        alpha: sim.alpha,
        seed: sim.seed,
        tick: sim.tick_no,
    };
    let q = Query {
        i,
        xi: sim.x[i as usize],
        yi: sim.y[i as usize],
    };
    let mut out = (0.0, 0.0);
    let mut frame = Frame {
        ctx: &ctx,
        q: &q,
        out: &mut out,
    };
    sim.charge_tree.visit_in(stack, |tree, node, bounds| {
        step(tree, node, bounds, &mut frame)
    });
    out
}

/// `ctx`, `q` and the accumulator bundled so [`step`] stays under the 4-param cap
/// (`refactor-rust.md`) despite the quadtree visit callback's own fixed 3 arguments.
pub(super) struct Frame<'a> {
    pub(super) ctx: &'a Ctx<'a>,
    pub(super) q: &'a Query,
    pub(super) out: &'a mut (f64, f64),
}

/// One quadtree node reached while querying node `q.i`: applies the Barnes-Hut
/// approximation when the opening angle allows it, else falls through to `direct`
/// (`manyBody.js`'s own `apply`).
pub(super) fn step(tree: &Quadtree, node: u32, bounds: Bounds, frame: &mut Frame) -> bool {
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
mod tests;
