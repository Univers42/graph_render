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

use super::sim::Sim;
use crate::layout::force::quadtree::{Bounds, Quadtree};
use crate::rng::jiggle;

const PASS_X: u32 = 4;
const PASS_Y: u32 = 5;

pub(super) fn apply(sim: &mut Sim) {
    for i in 0..sim.x.len() {
        sim.px[i] = sim.x[i] + sim.vx[i];
        sim.py[i] = sim.y[i] + sim.vy[i];
    }
    sim.collide_tree.build(&sim.px, &sim.py);
    sim.dvx.iter_mut().for_each(|v| *v = 0.0);
    sim.dvy.iter_mut().for_each(|v| *v = 0.0);
    let diameter = 2.0 * sim.params.collide_radius;
    let d2 = diameter * diameter;
    for i in 0..sim.x.len() {
        let (dvx, dvy) = node_delta(sim, i as u32, d2);
        sim.dvx[i] += dvx;
        sim.dvy[i] += dvy;
    }
    for i in 0..sim.vx.len() {
        sim.vx[i] += sim.dvx[i];
        sim.vy[i] += sim.dvy[i];
    }
}

struct Query<'a> {
    i: u32,
    xi: f64,
    yi: f64,
    d2: f64,
    seed: u32,
    tick: u32,
    px: &'a [f64],
    py: &'a [f64],
}

fn node_delta(sim: &mut Sim, i: u32, d2: f64) -> (f64, f64) {
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
    sim.collide_tree
        .visit(|tree, node, bounds| step(tree, node, bounds, &mut frame));
    out
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

/// `q.i`'s own half of the overlap correction against neighbour `p` (zero if they do
/// not overlap): a symmetric 50/50 split, since both share `collideRadius`.
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
