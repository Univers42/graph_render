//! The many-body (charge) force via Barnes-Hut approximation. Unlike link/collide this
//! is already a gather — each node reads the (already-built) quadtree and writes only
//! its own velocity, no partner to keep in step with — so it needs none of their Jacobi
//! double-buffering. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/manyBody.js`, simplified: every node carries
//! the same constant charge (`chargeStrength`, `params.rs`), so `accumulate`'s signed
//! `value` collapses to a plain point count times that constant, and a subtree's
//! `(x, y)` is its plain center of mass.
//!
//! The walk reads one [`Body`] per tree cell, in the tree's preorder (`quadtree/preorder.rs`):
//! a cell's centre, its opening threshold and the index past its subtree, 40 bytes read
//! front to back, so a query is a forward scan that jumps ahead instead of a stack.

use super::sim::Sim;
use crate::exec::Runner;
use crate::layout::force::quadtree::Quadtree;
use crate::rng::jiggle;

const PASS_X: u32 = 2;
const PASS_Y: u32 = 3;

/// The many-body pass, with its per-node gather divided by `runner` over `workers` workers.
///
/// `charge::apply_with(&Serial, 1, ..)` is the serial pass, and
/// `BarnesHut::run_with(&Serial, 1)` is the stage — one name, not three, so there is no
/// second spelling of the same pass to drift.
///
/// The merge ([`super::step::merge`]) puts each node's own delta on its own velocity: the
/// outputs are in the charge tree's point order, and one addition per node cannot pick up
/// a neighbour's term no matter how the gather was sliced.
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
    let order = Some(sim.charge_tree.order());
    super::step::merge((&mut sim.vx, &mut sim.vy), order, deltas, split);
}

/// The single-threaded prologue every many-body pass shares: build the quadtree over this
/// tick's positions, then aggregate masses and centres bottom-up.
///
/// Split out of [`apply_with`] because the range kernel ([`super::step::Pass`]) needs the
/// same two steps in front of it and must not have its own copy — a second tree build or a
/// second aggregate order would be a silent divergence from the serial pass, and the
/// equality test would then be comparing two programs rather than two schedules.
pub(super) fn prepare(sim: &mut Sim) {
    sim.charge_tree.build(&sim.x, &sim.y);
    let theta2 = sim.params.theta * sim.params.theta;
    aggregate(&sim.charge_tree, (&sim.x, &sim.y), theta2, &mut sim.bodies);
}

/// One tree cell as the walk reads it.
///
/// The mass is `count`, a point count: the `f64` sums it replaces were sums of whole
/// counts below 2^53, so exact, and `f64::from(count)` is the same number. `open` is
/// `w² / θ²`, the cell's opening threshold, computed once per tick instead of once per
/// visit; a query at squared distance `l` opens the cell when `open >= l`.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Body {
    comx: f64,
    comy: f64,
    open: f64,
    count: u32,
    skip: u32,
    start: u32,
}

/// Bottom-up mass and centre of mass per cell (`manyBody.js`'s `accumulate`), in reverse
/// preorder so every child is final before its parent reads it.
fn aggregate(tree: &Quadtree, (x, y): (&[f64], &[f64]), theta2: f64, bodies: &mut Vec<Body>) {
    let (cells, order) = (tree.cells(), tree.order());
    bodies.clear();
    bodies.resize(cells.len(), Body::default());
    for k in (0..cells.len()).rev() {
        let cell = cells[k];
        let (comx, comy) = if cell.skip == k as u32 + 1 {
            let head = order[cell.start as usize] as usize;
            (x[head], y[head])
        } else {
            centre(bodies, k as u32 + 1, cell.skip)
        };
        let w = cell.bounds.x1 - cell.bounds.x0;
        bodies[k] = Body {
            comx,
            comy,
            open: w * w / theta2,
            count: cell.end - cell.start,
            skip: cell.skip,
            start: cell.start,
        };
    }
}

/// The mass-weighted centre of the children in `first..skip`, in slot order. An internal
/// cell holds at least one point, so `m > 0`.
fn centre(bodies: &[Body], first: u32, skip: u32) -> (f64, f64) {
    let (mut m, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let mut c = first;
    while c < skip {
        let child = &bodies[c as usize];
        let cm = f64::from(child.count);
        m += cm;
        cx += cm * child.comx;
        cy += cm * child.comy;
        c = child.skip;
    }
    (cx / m, cy / m)
}

/// Everything a query reads but does not own, gathered once per range rather than once
/// per node.
pub(super) struct Ctx<'a> {
    bodies: &'a [Body],
    tree: &'a Quadtree,
    x: &'a [f64],
    y: &'a [f64],
    dmin2: f64,
    dmax2: f64,
    charge: f64,
    alpha: f64,
    seed: u32,
    tick: u32,
}

impl<'a> Ctx<'a> {
    /// The context over an already-prepared [`Sim`].
    pub(super) fn of(sim: &'a Sim) -> Self {
        Ctx {
            bodies: &sim.bodies,
            tree: &sim.charge_tree,
            x: &sim.x,
            y: &sim.y,
            dmin2: sim.params.distance_min * sim.params.distance_min,
            dmax2: sim.params.distance_max * sim.params.distance_max,
            charge: sim.params.charge,
            alpha: sim.alpha,
            seed: sim.seed,
            tick: sim.tick_no,
        }
    }
}

pub(super) struct Query {
    i: u32,
    xi: f64,
    yi: f64,
}

/// The offset from the query to a cell's centre and its squared length.
#[derive(Clone, Copy)]
struct Gap {
    dx: f64,
    dy: f64,
    l: f64,
}

/// Node `i`'s own many-body delta (`manyBody.js`'s `apply`, visited in `visit.js` order).
///
/// `&Ctx` only, so the range kernel ([`super::step::Pass`]) can have every worker walk the
/// same tree at once. A cell the opening angle resolves as one blob adds its delta and
/// jumps past its subtree; a cell too close descends, or, at a leaf, adds its chain's
/// exact terms. The test is written `open >= l` with the approximation in the `else`,
/// as `manyBody.js` writes it, so a NaN distance approximates as it always did.
pub(super) fn node_delta(ctx: &Ctx, i: u32) -> (f64, f64) {
    let q = Query {
        i,
        xi: ctx.x[i as usize],
        yi: ctx.y[i as usize],
    };
    let mut out = (0.0, 0.0);
    let mut k = 0;
    while let Some(body) = ctx.bodies.get(k as usize) {
        let (dx, dy) = (body.comx - q.xi, body.comy - q.yi);
        let gap = Gap {
            dx,
            dy,
            l: dx * dx + dy * dy,
        };
        let (delta, next) = if body.open >= gap.l {
            if body.skip != k + 1 {
                k += 1;
                continue;
            }
            (direct(ctx, &q, body, gap), k + 1)
        } else {
            (approx(ctx, &q, k, gap), body.skip)
        };
        out.0 += delta.0;
        out.1 += delta.1;
        k = next;
    }
    out
}

/// Cell `k` resolved as one blob of its whole mass; zero past `distanceMax`. The jiggle is
/// keyed on the cell's insertion-order node id, the key it has always had.
fn approx(ctx: &Ctx, q: &Query, k: u32, gap: Gap) -> (f64, f64) {
    let Some(Gap { dx, dy, l }) = settle(ctx, q, ctx.tree.key(k), gap) else {
        return (0.0, 0.0);
    };
    let m = f64::from(ctx.bodies[k as usize].count);
    let f = m * ctx.charge * ctx.alpha / l;
    (dx * f, dy * f)
}

/// A leaf too close (or too coarse) to approximate: every distinct chained point
/// contributes its own charge directly (`manyBody.js`'s coincidence-chain loop), unless
/// `distanceMax` has already been reached, in which case it contributes nothing — the
/// same cutoff `approx` applies to the Barnes-Hut-approximable case (`manyBody.js:77`:
/// `else if (quad.length || l >= distanceMax2) return;`). A leaf's centre is its chain
/// head's position, so `gap` is the offset to every chained point.
fn direct(ctx: &Ctx, q: &Query, body: &Body, gap: Gap) -> (f64, f64) {
    let chain = &ctx.tree.order()[body.start as usize..][..body.count as usize];
    let Some(Gap { dx, dy, l }) = settle(ctx, q, chain[0], gap) else {
        return (0.0, 0.0);
    };
    let (mut dvx, mut dvy) = (0.0, 0.0);
    for &p in chain {
        if p == q.i {
            continue;
        }
        let f = ctx.charge * ctx.alpha / l;
        dvx += dx * f;
        dvy += dy * f;
    }
    (dvx, dvy)
}

/// The tail both ends of `manyBody.js`'s `apply` share: `None` past `distanceMax`, else the
/// gap with an exactly-zero axis jiggled (keyed on `(q.i, key)`) and `l` clamped up to
/// `distanceMin`.
fn settle(ctx: &Ctx, q: &Query, key: u32, gap: Gap) -> Option<Gap> {
    let Gap {
        mut dx,
        mut dy,
        mut l,
    } = gap;
    if l >= ctx.dmax2 {
        return None;
    }
    if dx == 0.0 {
        dx = jiggle(ctx.seed, ctx.tick, PASS_X, (q.i, key));
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(ctx.seed, ctx.tick, PASS_Y, (q.i, key));
        l += dy * dy;
    }
    if l < ctx.dmin2 {
        l = f64::sqrt(ctx.dmin2 * l);
    }
    Some(Gap { dx, dy, l })
}

#[cfg(test)]
mod tests;
