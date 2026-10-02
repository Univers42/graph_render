//! Circle packing by force relaxation (`_circle_packing_force_directed`,
//! `circle_packing.py:407-542`): the path SciGraphs takes for a graph its
//! Collins–Stephenson packer cannot take. Adjacent circles are pulled toward tangency and
//! every overlapping pair is pushed apart, but neither is guaranteed to be reached — this
//! is [`super`]'s degraded output, always flagged with note code 3
//! (`docs/decisions/planarity-fallback.md`).
//!
//! Two deviations from SciGraphs, both required by graph-core's own house limits and
//! both recorded, not silently applied: no RNG ([`seed`]'s golden-spiral seed replaces
//! `nx.spring_layout`'s random one), and no `cKDTree` (every close-pair scan below is the
//! deterministic ascending-`(i, j)` scan SciGraphs itself falls back to without SciPy,
//! `circle_packing.py:400-405`).

mod relax;
mod seed;

use super::CirclePackingParams;
use super::geometry::{Packed, center, fit_to_scale};
use relax::{RelaxParams, relax};
use seed::{fruchterman_reingold, rescale_to};

/// The fallback's entry point: seeds positions, relaxes them, and always reports
/// `approximate = true`.
///
/// `edges` arrives already reduced to a simple graph by [`super::simple_pairs`] — the
/// same reduction the exact path gets — so every edge here is a distinct pair of distinct
/// nodes: one spring each, and a degree that counts each incident edge once.
pub(super) fn pack(
    n: u32,
    edges: &[(u32, u32)],
    loops: &[u32],
    params: &CirclePackingParams,
) -> Packed {
    let scale = f64::from(params.scale);
    let radii = initial_radii(n, edges, loops, scale);
    let mut seeded = fruchterman_reingold(n, edges, seed_iterations(n));
    rescale_to(&mut seeded, scale * 0.45);
    let relax_params = RelaxParams {
        iterations: params.iterations.max(1),
        scale,
    };
    let (mut positions, mut radii) = relax(seeded, radii, edges, &relax_params);
    center(&mut positions);
    fit_to_scale(&mut positions, &mut radii, scale);
    let (x, y) = positions.into_iter().unzip();
    Packed {
        x,
        y,
        r: radii,
        approximate: true,
    }
}

/// SciGraphs' own seed budget, `max(10, min(50, 20000 // max(num_nodes, 1)))`
/// (`circle_packing.py:427`): the node count floored into the divisor, the quotient
/// floored by the reference's own integer division, then a two-sided clamp to `[10, 50]`.
/// The integer division is spelled as such rather than left to an `f64` one because the
/// reference's is; the two agree on every `n`, since the quotient's fractional part is
/// under 1 and so `q` and `floor(q)` cross 10 and 50 at the same `n`.
fn seed_iterations(n: u32) -> u32 {
    (20_000 / n.max(1)).clamp(10, 50)
}

/// Degree-proportional starting radii, normalised so their squares sum to `0.35` of the
/// frame's area (`circle_packing.py:420-425`). Each incident edge counts once, the same
/// convention `G.degree` uses on a simple graph — and on the simple graph
/// [`super::simple_pairs`] hands over, a self-loop is already gone, so there is nothing
/// here for the `u == v` case to double count.
fn initial_radii(n: u32, edges: &[(u32, u32)], loops: &[u32], scale: f64) -> Vec<f64> {
    let mut degree = vec![0.0_f64; n as usize];
    for &(u, v) in edges {
        degree[u as usize] += 1.0;
        degree[v as usize] += 1.0;
    }
    for (node, &count) in loops.iter().enumerate() {
        degree[node] += 2.0 * f64::from(count);
    }
    let max_degree = degree.iter().cloned().fold(1.0, f64::max);
    let mut radii: Vec<f64> = degree
        .iter()
        .map(|&d| 0.3 + 0.7 * (d / max_degree))
        .collect();
    let frame = scale * 0.45;
    let sum_sq: f64 = radii.iter().map(|r| r * r).sum();
    if sum_sq > 0.0 {
        let factor = libm::sqrt(0.35 * frame * frame / sum_sq);
        for r in &mut radii {
            *r *= factor;
        }
    }
    radii
}

/// How many self-loops each node carries, counted separately from `edges` because
/// [`super::simple_pairs`] reduces them away before [`pack`] is reached, and networkx's
/// `G.degree` counts each of them twice (`reportviews.py:526`) on the graph
/// `_build_networkx_graph` built with them still in (`common.py:297`).
fn loop_degrees(edges: &[(u32, u32)]) -> Vec<u32> {
    let mut loops = Vec::new();
    for &(u, v) in edges {
        if u != v {
            continue;
        }
        if loops.len() <= u as usize {
            loops.resize(u as usize + 1, 0);
        }
        loops[u as usize] += 1;
    }
    loops
}

/// `diff`/`dist` for the pair `(u, v)`, with a deterministic direction substituted for a
/// coincident pair (Ponytail: SciGraphs jitters that case with `rng.rand() - 0.5`;
/// `nudge` below is this port's fixed, index-derived replacement — direction: an
/// arbitrary but repeatable separating direction, never a wrong one, since the pair was
/// already at distance zero and any direction separates it).
pub(super) fn separated(raw: (f64, f64), u: u32, v: u32) -> ((f64, f64), f64) {
    let dist = libm::hypot(raw.0, raw.1);
    if dist < 1e-6 {
        (nudge(u, v), 1.0)
    } else {
        (raw, dist)
    }
}

pub(super) fn nudge(u: u32, v: u32) -> (f64, f64) {
    let angle = f64::from(u) * seed::GOLDEN_ANGLE + f64::from(v) * core::f64::consts::PI;
    (libm::cos(angle), libm::sin(angle))
}

#[cfg(test)]
mod tests;
