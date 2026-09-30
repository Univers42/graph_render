//! The collide force as a Jacobi gather (devil C7): each node independently visits the
//! (already-built) quadtree over this tick's *projected* positions (`x + vx`, matching
//! `collide.js`'s own quadtree keys) and computes its own share of every overlap it
//! finds — accumulated into `dvx`/`dvy` and merged only once every node has been read,
//! so no node's query sees another's still-in-progress update. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/collide.js`, simplified: the frozen params
//! give every node the same `collideRadius`, so d3's own per-pair split
//! `rj²/(ri²+rj²)` is exactly one half either way, and both directions of an overlap
//! (`i` querying and finding `j`, `j` querying and finding `i`) derive their delta from
//! the same subtraction negated (`b - a == -(a - b)` exactly, in IEEE754), which is this
//! port's realisation of "canonical orientation": one shared difference, read from
//! either end, never independently re-derived.
//!
//! **The range kernel.** Node `i`'s share of every overlap it finds depends on nothing
//! but start-of-step state — this tick's projected positions, the built quadtree, the
//! frozen radius, the seed and the tick — and it is written only to `out[i]`. So the pass
//! is a gather in the D10 sense and partitions by node, the same shape as `charge`: see
//! [`super::step::CollidePass`] for the contract and `docs/decisions/link-gather.md` for
//! why `link` needed an argument and this one did not.

use super::sim::Sim;
use super::step::CollidePass;
use crate::exec::Runner;
use crate::layout::force::quadtree::{Bounds, Quadtree};
use crate::rng::jiggle;

const PASS_X: u32 = 4;
const PASS_Y: u32 = 5;

/// The collide pass, with its per-node gather divided by `runner` over `workers` workers.
///
/// `apply_with(&Serial, 1, ..)` is the serial pass, which is what the stage runs in every
/// tier's reference arm. `split` is this pass's own slice of the negative control: it
/// makes the merge read the next node's delta as well, the shape a wrong partition of the
/// outputs would take.
pub(super) fn apply_with(
    sim: &mut Sim,
    runner: &impl Runner,
    workers: u32,
    deltas: &mut Vec<(f64, f64)>,
    split: bool,
) {
    prepare(sim);
    runner.run(&CollidePass::of(&*sim), workers, deltas);
    merge(sim, deltas, split);
}

/// The single-threaded prologue: this tick's projected positions, the quadtree over them,
/// and the reach every node resolves against. Built once, ahead of the ranges, and shared
/// by the serial and the threaded path — a second build would be a different arena order,
/// so it would be a divergence rather than a schedule.
pub(super) fn prepare(sim: &mut Sim) {
    for i in 0..sim.x.len() {
        sim.px[i] = sim.x[i] + sim.vx[i];
        sim.py[i] = sim.y[i] + sim.vy[i];
    }
    sim.collide_tree.build(&sim.px, &sim.py);
}

/// `(2 * collideRadius)²`: the squared diameter two nodes must be closer than to overlap.
/// Derived from the parameters once per pass rather than per node.
pub(super) fn reach_squared(sim: &Sim) -> f64 {
    let diameter = 2.0 * sim.params.collide_radius;
    diameter * diameter
}

/// `vx[i] += deltas[i]`, in ascending node index — and, under the control, the next node's
/// delta too.
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

/// Node `i`'s own delta over the prepared [`Sim`], into a caller's reused walk stack.
///
/// `&self` and a borrowed stack, so the range kernel can hold one `&Sim` while several
/// workers walk the same tree at once — the walk is iterative precisely so its buffer can
/// be borrowed rather than owned (`quadtree.rs`'s `visit_in`).
pub(super) fn node_delta(sim: &Sim, i: u32, d2: f64, stack: &mut Vec<(u32, Bounds)>) -> (f64, f64) {
    let q = Query {
        i,
        xi: sim.px[i as usize],
        yi: sim.py[i as usize],
        d2,
        seed: sim.seed,
        tick: sim.tick_no,
        px: &sim.px,
        py: &sim.py,
    };
    let mut out = (0.0, 0.0);
    let mut frame = Frame {
        q: &q,
        out: &mut out,
    };
    sim.collide_tree.visit_in(stack, |tree, node, bounds| {
        step(tree, node, bounds, &mut frame)
    });
    out
}

/// Everything node `q.i`'s query reads but does not own: its projected position, the reach,
/// the jiggle's seed and tick, and the projected columns themselves.
pub(super) struct Query<'a> {
    i: u32,
    xi: f64,
    yi: f64,
    d2: f64,
    seed: u32,
    tick: u32,
    px: &'a [f64],
    py: &'a [f64],
}

/// `q` and the accumulator bundled so [`step`] stays under the 4-param cap
/// (`refactor-rust.md`) despite the quadtree visit callback's own fixed 3 arguments.
struct Frame<'a> {
    q: &'a Query<'a>,
    out: &'a mut (f64, f64),
}

/// One quadtree node reached while querying node `q.i`'s neighbourhood: prunes a
/// quadrant entirely outside the collision reach, else resolves a leaf found inside it.
fn step(tree: &Quadtree, node: u32, bounds: Bounds, frame: &mut Frame) -> bool {
    if tree.children(node).is_some() {
        let reach = libm::sqrt(frame.q.d2);
        return bounds.x0 > frame.q.xi + reach
            || bounds.x1 < frame.q.xi - reach
            || bounds.y0 > frame.q.yi + reach
            || bounds.y1 < frame.q.yi - reach;
    }
    for p in tree.leaf_points(node) {
        if p != frame.q.i {
            resolve(frame.q, p, frame.out);
        }
    }
    false
}

/// `q.i`'s own half of the overlap correction against neighbour `p` (zero if they do not
/// overlap): a symmetric 50/50 split, since both share `collideRadius`.
fn resolve(q: &Query, p: u32, out: &mut (f64, f64)) {
    let mut dx = q.xi - q.px[p as usize];
    let mut dy = q.yi - q.py[p as usize];
    let mut l = dx * dx + dy * dy;
    if l >= q.d2 {
        return;
    }
    if dx == 0.0 {
        dx = jiggle(q.seed, q.tick, PASS_X, (q.i, p));
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(q.seed, q.tick, PASS_Y, (q.i, p));
        l += dy * dy;
    }
    let dist = libm::sqrt(l);
    let push = (libm::sqrt(q.d2) - dist) / dist * 0.5;
    out.0 += dx * push;
    out.1 += dy * push;
}
