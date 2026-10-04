//! The link force as a Jacobi gather (devil C7): a shared `x`/`vx`/`y`/`vy` snapshot for
//! the whole pass (accumulated into `dvx`/`dvy`, merged only once every edge has been
//! read), and each edge's delta computed exactly once and split between its two
//! endpoints by their degree bias. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/link.js`, with one documented deviation:
//! d3's bias is `count[source]/(count[source]+count[target])`, `source`/`target` being
//! the raw graph's own arbitrary direction; our shared [`SimpleGraph`] (devil C6) has
//! already collapsed that direction, so "source" and "target" are redefined here as the
//! pair's lower and higher node index respectively — a fixed, deterministic stand-in,
//! not the original edge's own orientation.
//!
//! **Why this pass is a gather when charge's already was.** An edge's force has *two*
//! endpoints: computing it once and adding it into both is a scatter, which D10 forbids,
//! so a naive port of this loop into a range kernel would have one range write into
//! another's slot. `docs/decisions/link-gather.md` states the argument this port is built
//! on: partition by **node**, recompute the pair's
//! one shared difference from either end — bit-identical, since IEEE754 subtraction is
//! antisymmetric — and sum each node's own share over its own incident edges, in the row
//! order [`SimpleGraph`] already fixes.

use super::sim::Sim;
use super::step::{LinkForces, LinkPass};
use crate::exec::Runner;
use crate::layout::force::LiveParams;
use crate::layout::force::SimpleGraph;
use crate::rng::jiggle;

const PASS_X: u32 = 0;
const PASS_Y: u32 = 1;

/// Each simple edge's fixed `(distance, strength, bias)` (`forceLayout.ts:206-207`):
/// the topology never changes across ticks, so neither do these — but the *parameters*
/// can, mid-run, so this is recomputed whenever they are replaced
/// ([`Sim::set_params`]).
pub(super) fn geometry(graph: &SimpleGraph, params: &LiveParams) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let m = graph.lo.len();
    let (mut distance, mut strength, mut bias) = (
        Vec::with_capacity(m),
        Vec::with_capacity(m),
        Vec::with_capacity(m),
    );
    for e in 0..m {
        let s = graph.strength[e];
        distance.push(params.link_distance / f64::max(0.4, s));
        strength.push(f64::min(0.7, params.link_strength_scale * s));
        let (dlo, dhi) = (
            f64::from(graph.degree(graph.lo[e])),
            f64::from(graph.degree(graph.hi[e])),
        );
        bias.push(dlo / (dlo + dhi));
    }
    (distance, strength, bias)
}

/// One Jacobi pass over every simple edge (`link.js`'s own `iterations` defaults to,
/// and here is fixed at, 1), with the per-node gather divided by `runner` over `workers`
/// workers.
///
/// `split` is this pass's own slice of the negative control: it makes the merge read the
/// next node's delta as well, the shape a wrong partition of the outputs would take.
pub(in crate::layout::force) fn apply_with(
    sim: &mut Sim,
    runner: &impl Runner,
    workers: u32,
    deltas: &mut Vec<(f64, f64)>,
    split: bool,
) {
    pass_with(sim, runner, workers, deltas);
    super::step::merge((&mut sim.vx, &mut sim.vy), None, deltas, split);
}

/// [`apply_with`]'s deltas in node order, not yet merged: the particle mesh merges them
/// as a range pass (`particle_mesh/motion.rs`).
pub(in crate::layout::force) fn pass_with(
    sim: &mut Sim,
    runner: &impl Runner,
    workers: u32,
    deltas: &mut Vec<(f64, f64)>,
) {
    let mut forces = std::mem::take(&mut sim.link_forces);
    runner.run(&LinkForces::of(sim), workers, &mut forces);
    runner.run(&LinkPass::of(sim, &forces), workers, deltas);
    sim.link_forces = forces;
}

/// Simple edge `e`'s force before the bias splits it, in `(x, y)`: the one square root and
/// division of the edge, computed once per tick by [`LinkForces`].
///
/// **Why the `l` below is divided by at all, and when it is zero.** `jiggle` no longer returns
/// exactly `+0.0` — `rng::jiggle_of` maps its one midpoint word
/// away — so both `jiggle` branches in `force` install a non-zero axis and a fully coincident
/// pair has `l > 0`. That is what item 1 of this repair buys, and
/// `a_link_between_two_coincident_nodes_has_a_finite_force` tests it. The remaining way to
/// reach `l == 0.0` is not coincidence but **magnitude**: `dx * dx` underflows for any axis
/// below `sqrt(f64::MIN_POSITIVE) ≈ 1.5e-162`, so a pair `1e-200` apart is not coincident,
/// takes no `jiggle` branch, and still sums to `0.0`. So the denominator is *not* provably
/// non-zero, and the division is floored the way `charge.rs`'s is.
pub(super) fn force(sim: &Sim, e: usize) -> (f64, f64) {
    let (lo, hi) = (sim.graph.lo[e] as usize, sim.graph.hi[e] as usize);
    let (mut dx, mut dy) = displaced(sim, hi, lo);
    if dx == 0.0 {
        dx = jiggle(sim.seed, sim.tick_no, PASS_X, (lo as u32, hi as u32));
    }
    if dy == 0.0 {
        dy = jiggle(sim.seed, sim.tick_no, PASS_Y, (lo as u32, hi as u32));
    }
    let l = f64::sqrt(dx * dx + dy * dy);
    // A separation whose square underflowed, as the doc comment above says.
    // Ponytail: the floor contributes nothing for such a pair, so a link shorter than
    // ~1.5e-162 gets no spring at all — what it gets wrong is dropping the push that would
    // separate the two, never reporting a wrong one. The direction of that push is below
    // `f64` resolution anyway, and charge.rs's `distanceMin²` floor divides to ~1e106 here.
    // Escape hatch: distinct coordinates, which the two `jiggle` branches already give to
    // anything at the same point.
    if l == 0.0 {
        return (0.0, 0.0);
    }
    let factor = (l - sim.link_distance[e]) / l * sim.alpha * sim.link_strength[e];
    (dx * factor, dy * factor)
}

/// The share of an edge's force `(fx, fy)` that moves its higher endpoint when `hi`, else
/// its lower one, weighted by the edge's bias `b`.
pub(super) fn share((fx, fy): (f64, f64), b: f64, hi: bool) -> (f64, f64) {
    if hi {
        (-fx * b, -fy * b)
    } else {
        (fx * (1.0 - b), fy * (1.0 - b))
    }
}

/// Simple edge `e`'s two halves of the force, in `(x, y)`: the share that moves its lower
/// endpoint and the share that moves its higher one.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the serial reference's own split, which only tests call"
    )
)]
pub(super) fn halves(sim: &Sim, e: usize) -> ((f64, f64), (f64, f64)) {
    let (f, b) = (force(sim, e), sim.link_bias[e]);
    (share(f, b, false), share(f, b, true))
}

/// Edge `e`'s delta scattered into both its endpoints, at `out[lo]` and `out[hi]`.
///
/// **The serial reference for [`LinkPass`], and the shape D10 forbids inside a range**: it
/// writes two elements that may fall in two different ranges. It stays as a named
/// function because it *is* what the kernel must reproduce — one edge's contribution,
/// identically ordered from either end — and because a test compares the two.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the serial reference a test compares the kernel to"
    )
)]
pub(super) fn scatter(sim: &Sim, e: usize, out: &mut [(f64, f64)]) {
    let (lo, hi) = (sim.graph.lo[e] as usize, sim.graph.hi[e] as usize);
    let ((lox, loy), (hix, hiy)) = halves(sim, e);
    out[lo].0 += lox;
    out[lo].1 += loy;
    out[hi].0 += hix;
    out[hi].1 += hiy;
}

/// `x[a] + vx[a]` minus `x[b] + vx[b]`: the pair's one shared difference, read from the
/// higher endpoint. Both endpoints of an edge compute it here, so the subtraction is
/// performed once per direction and is antisymmetric in IEEE754 — `a - b` and `b - a` are
/// exact negations, including the `+0.0` each becomes when the two coincide, which is
/// exactly the case the `jiggle` branch below then replaces.
fn displaced(sim: &Sim, hi: usize, lo: usize) -> (f64, f64) {
    (
        (sim.x[hi] + sim.vx[hi]) - (sim.x[lo] + sim.vx[lo]),
        (sim.y[hi] + sim.vy[hi]) - (sim.y[lo] + sim.vy[lo]),
    )
}

#[cfg(test)]
mod tests;
