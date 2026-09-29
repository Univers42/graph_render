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
use super::step::LinkPass;
use crate::exec::Runner;
use crate::layout::force::SimpleGraph;
use crate::layout::force::params::ForceParams;
use crate::rng::jiggle;

const PASS_X: u32 = 0;
const PASS_Y: u32 = 1;

/// Each simple edge's fixed `(distance, strength, bias)` (`forceLayout.ts:206-207`):
/// the topology never changes across ticks, so neither do these.
pub(super) fn geometry(
    graph: &SimpleGraph,
    params: &ForceParams,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
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
pub(super) fn apply_with(
    sim: &mut Sim,
    runner: &impl Runner,
    workers: u32,
    deltas: &mut Vec<(f64, f64)>,
    split: bool,
) {
    runner.run(&LinkPass::of(&*sim), workers, deltas);
    merge(sim, deltas, split);
}

/// `vx[i] += deltas[i]` in ascending node index, and the same for `y` — the merge every
/// gathered pass ends with, in the same shape as `charge::merge`, so a partition mistake
/// shows up in the same place for all three.
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

/// Simple edge `e`'s two halves of the force, in `(x, y)`: the share that moves its higher
/// endpoint and the share that moves its lower one, already weighted by the edge's bias.
pub(super) fn halves(sim: &Sim, e: usize) -> ((f64, f64), (f64, f64)) {
    let (lo, hi) = (sim.graph.lo[e] as usize, sim.graph.hi[e] as usize);
    let (mut dx, mut dy) = displaced(sim, hi, lo);
    if dx == 0.0 {
        dx = jiggle(sim.seed, sim.tick_no, PASS_X, (lo as u32, hi as u32));
    }
    if dy == 0.0 {
        dy = jiggle(sim.seed, sim.tick_no, PASS_Y, (lo as u32, hi as u32));
    }
    let l = libm::sqrt(dx * dx + dy * dy);
    let factor = (l - sim.link_distance[e]) / l * sim.alpha * sim.link_strength[e];
    let (fx, fy) = (dx * factor, dy * factor);
    let b = sim.link_bias[e];
    ((fx * (1.0 - b), fy * (1.0 - b)), (-fx * b, -fy * b))
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
