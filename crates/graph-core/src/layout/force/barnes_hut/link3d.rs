//! The 3D link force: the 2D pass's Jacobi gather over three axes, with no range kernel.
//!
//! The shape is `link.rs`'s, unchanged: each simple edge's force is computed **once** and
//! split between its two endpoints by the degree bias, and each node gathers its own share
//! over its own CSR row in that row's order. What the 2D pass proves with a
//! [`StepRange`](crate::exec::StepRange) — that the schedule moves no byte — has nothing to
//! prove here, because there is exactly one schedule (see `sim3d.rs`'s header). The
//! arithmetic, the jiggle branches, the `l == 0` floor and the `>=`/`1 - b` share split are
//! the 2D ones with a third term.

use super::sim3d::Sim3;
use crate::rng::jiggle;

const PASS_X: u32 = 8;
const PASS_Y: u32 = 9;
const PASS_Z: u32 = 10;

/// The whole pass: one force per edge, then one gather per node, then one addition onto
/// each node's own velocity.
pub(in crate::layout::force) fn apply(sim: &mut Sim3) {
    let m = sim.graph.lo.len();
    sim.link_forces.resize(m, (0.0, 0.0, 0.0));
    for e in 0..m {
        sim.link_forces[e] = force(sim, e);
    }
    for i in 0..sim.rows() {
        let d = node_share(sim, i);
        let i = i as usize;
        sim.vx[i] += d.0;
        sim.vy[i] += d.1;
        sim.vz[i] += d.2;
    }
}

/// Simple edge `e`'s force before the bias splits it, in `(x, y, z)`: one square root and
/// one division for the edge.
///
/// **Why the denominator is floored.** `jiggle` does not return exactly `+0.0`, so both
/// jiggle branches here install a non-zero axis and a fully coincident pair has `l > 0`. The
/// remaining way to `l == 0.0` is magnitude: `dx * dx` underflows for any axis below
/// `sqrt(f64::MIN_POSITIVE)`, so a pair `1e-200` apart is not coincident, takes no jiggle
/// branch, and still sums to zero.
pub(in crate::layout::force) fn force(sim: &Sim3, e: usize) -> (f64, f64, f64) {
    let (lo, hi) = (sim.graph.lo[e] as usize, sim.graph.hi[e] as usize);
    let (mut dx, mut dy, mut dz) = displaced(sim, hi, lo);
    let key = (lo as u32, hi as u32);
    if dx == 0.0 {
        dx = jiggle(sim.seed, sim.tick_no, PASS_X, key);
    }
    if dy == 0.0 {
        dy = jiggle(sim.seed, sim.tick_no, PASS_Y, key);
    }
    if dz == 0.0 {
        dz = jiggle(sim.seed, sim.tick_no, PASS_Z, key);
    }
    let l = f64::sqrt(dx * dx + dy * dy + dz * dz);
    // Ponytail: the floor contributes nothing for such a pair, so a link shorter than
    // ~1.5e-162 gets no spring at all — what it gets wrong is dropping the push that would
    // separate the two, never reporting a wrong one. Escape hatch: distinct coordinates,
    // which the three jiggle branches already give to anything at the same point.
    if l == 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let factor = (l - sim.link_distance[e]) / l * sim.alpha * sim.link_strength[e];
    (dx * factor, dy * factor, dz * factor)
}

/// The share of an edge's force that moves its higher endpoint when `hi`, else its lower
/// one, weighted by the edge's bias `b`.
fn share(f: (f64, f64, f64), b: f64, hi: bool) -> (f64, f64, f64) {
    if hi {
        (-f.0 * b, -f.1 * b, -f.2 * b)
    } else {
        (f.0 * (1.0 - b), f.1 * (1.0 - b), f.2 * (1.0 - b))
    }
}

/// Node `i`'s own share of every simple edge incident to it, in its row's order — the 2D
/// `LinkPass`'s `node_share`, at three axes.
pub(in crate::layout::force) fn node_share(sim: &Sim3, i: u32) -> (f64, f64, f64) {
    let mut dv = (0.0, 0.0, 0.0);
    for &e in sim.graph.rows.row(i) {
        let e = e as usize;
        let hi = sim.graph.hi[e] == i;
        let s = share(sim.link_forces[e], sim.link_bias[e], hi);
        dv.0 += s.0;
        dv.1 += s.1;
        dv.2 += s.2;
    }
    dv
}

/// `p[a] + v[a]` minus `p[b] + v[b]`, over all three axes: the pair's one shared
/// difference, read from the higher endpoint and antisymmetric in IEEE754, so both
/// endpoints compute the same bytes.
fn displaced(sim: &Sim3, hi: usize, lo: usize) -> (f64, f64, f64) {
    (
        (sim.x[hi] + sim.vx[hi]) - (sim.x[lo] + sim.vx[lo]),
        (sim.y[hi] + sim.vy[hi]) - (sim.y[lo] + sim.vy[lo]),
        (sim.z[hi] + sim.vz[hi]) - (sim.z[lo] + sim.vz[lo]),
    )
}
