//! Circle packing by force relaxation (`_circle_packing_force_directed`,
//! `circle_packing.py:407-503`): the path SciGraphs takes for a graph its
//! Collins–Stephenson packer cannot take. Adjacent circles are pulled toward tangency and
//! every overlapping pair is pushed apart, but neither is guaranteed to be reached — this
//! is [`super`]'s degraded output, always flagged with note code 3
//! (`docs/decisions/planarity-fallback.md`).
//!
//! Two deviations from SciGraphs, both required by graph-core's own house limits and
//! both recorded, not silently applied: no RNG ([`seed`]'s golden-spiral seed replaces
//! `nx.spring_layout`'s random one), and no `cKDTree` (every close-pair scan below is the
//! deterministic ascending-`(i, j)` scan SciGraphs itself falls back to without SciPy,
//! `circle_packing.py:398-405`).

mod seed;

use super::CirclePackingParams;
use super::geometry::{Packed, center, fit_to_scale};
use seed::{fruchterman_reingold, rescale_to};

/// The fallback's entry point: seeds positions, relaxes them, and always reports
/// `approximate = true`.
pub(super) fn pack(n: u32, edges: &[(u32, u32)], params: &CirclePackingParams) -> Packed {
    let scale = f64::from(params.scale);
    let radii = initial_radii(n, edges, scale);
    let simple: Vec<(u32, u32)> = edges.iter().copied().filter(|&(u, v)| u != v).collect();
    let seed_iterations = (20_000 / n.max(1)).clamp(10, 50);
    let mut seeded = fruchterman_reingold(n, &simple, seed_iterations);
    rescale_to(&mut seeded, scale * 0.45);
    let relax_params = RelaxParams {
        iterations: params.iterations.max(1),
        scale,
    };
    let (mut positions, mut radii) = relax(seeded, radii, &simple, &relax_params);
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

/// Degree-proportional starting radii, normalised so their squares sum to `0.35` of the
/// frame's area (`circle_packing.py:419-424`). Degree counts a self-loop twice, the same
/// convention `G.degree` uses, which falls out here for free: a self-loop's `u == v`
/// increments the same slot on both sides of the pair.
fn initial_radii(n: u32, edges: &[(u32, u32)], scale: f64) -> Vec<f64> {
    let mut degree = vec![0.0_f64; n as usize];
    for &(u, v) in edges {
        degree[u as usize] += 1.0;
        degree[v as usize] += 1.0;
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

/// `relax`'s own tunables, bundled so it stays under the parameter cap.
struct RelaxParams {
    iterations: u32,
    scale: f64,
}

/// Read-only state every per-pair force needs: bundled for the same reason as
/// [`RelaxParams`].
struct RelaxField<'a> {
    radii: &'a [f64],
    edge_keys: &'a [u64],
    cutoff: f64,
    n: u32,
}

impl RelaxField<'_> {
    fn is_edge(&self, i: u32, j: u32) -> bool {
        self.edge_keys
            .binary_search(&edge_key(self.n, i, j))
            .is_ok()
    }
}

fn edge_key(n: u32, i: u32, j: u32) -> u64 {
    let (lo, hi) = if i < j { (i, j) } else { (j, i) };
    u64::from(lo) * u64::from(n) + u64::from(hi)
}

fn edge_key_set(n: u32, edges: &[(u32, u32)]) -> Vec<u64> {
    let mut keys: Vec<u64> = edges.iter().map(|&(u, v)| edge_key(n, u, v)).collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Edge-pull-then-overlap-push relaxation, cooling like simulated annealing, then a
/// pure-overlap settle pass (`circle_packing.py:437-503`). Neither goal is guaranteed,
/// unlike the exact path.
fn relax(
    mut positions: Vec<(f64, f64)>,
    radii: Vec<f64>,
    edges: &[(u32, u32)],
    p: &RelaxParams,
) -> (Vec<(f64, f64)>, Vec<f64>) {
    let n = positions.len() as u32;
    let edge_keys = edge_key_set(n, edges);
    let cutoff = 2.2 * radii.iter().cloned().fold(0.0_f64, f64::max);
    let field = RelaxField {
        radii: &radii,
        edge_keys: &edge_keys,
        cutoff,
        n,
    };
    let mut temperature = p.scale * 0.2;
    for _ in 0..p.iterations {
        let mut forces = vec![(0.0, 0.0); n as usize];
        edge_pull(&positions, &field, edges, &mut forces);
        overlap_and_repel(&positions, &field, &mut forces);
        cap_and_apply(&mut positions, &forces, temperature);
        temperature *= 0.98;
        if temperature < 1e-4 {
            break;
        }
    }
    settle(&mut positions, &field, p.iterations);
    (positions, radii)
}

/// `diff`/`dist` for the pair `(u, v)`, with a deterministic direction substituted for a
/// coincident pair (Ponytail: SciGraphs jitters that case with `rng.rand() - 0.5`;
/// `nudge` below is this port's fixed, index-derived replacement — direction: an
/// arbitrary but repeatable separating direction, never a wrong one, since the pair was
/// already at distance zero and any direction separates it).
fn separated(raw: (f64, f64), u: u32, v: u32) -> ((f64, f64), f64) {
    let dist = libm::hypot(raw.0, raw.1);
    if dist < 1e-6 {
        (nudge(u, v), 1.0)
    } else {
        (raw, dist)
    }
}

fn nudge(u: u32, v: u32) -> (f64, f64) {
    let angle = f64::from(u) * seed::GOLDEN_ANGLE + f64::from(v) * core::f64::consts::PI;
    (libm::cos(angle), libm::sin(angle))
}

/// Spring pull along every graph edge toward its target tangency (`circle_packing.py:437-445`).
fn edge_pull(
    positions: &[(f64, f64)],
    field: &RelaxField,
    edges: &[(u32, u32)],
    forces: &mut [(f64, f64)],
) {
    for &(u, v) in edges {
        let target = field.radii[u as usize] + field.radii[v as usize];
        let raw = (
            positions[v as usize].0 - positions[u as usize].0,
            positions[v as usize].1 - positions[u as usize].1,
        );
        let (diff, dist) = separated(raw, u, v);
        let pull = (dist - target) * 0.3 / dist;
        forces[u as usize].0 += pull * diff.0;
        forces[u as usize].1 += pull * diff.1;
        forces[v as usize].0 -= pull * diff.0;
        forces[v as usize].1 -= pull * diff.1;
    }
}

/// Every close pair pushed apart if it overlaps, or weakly repelled if it neither
/// overlaps nor shares a graph edge (`circle_packing.py:455-475`).
fn overlap_and_repel(positions: &[(f64, f64)], field: &RelaxField, forces: &mut [(f64, f64)]) {
    for i in 0..field.n {
        for j in (i + 1)..field.n {
            let raw = (
                positions[j as usize].0 - positions[i as usize].0,
                positions[j as usize].1 - positions[i as usize].1,
            );
            let (diff, dist) = separated(raw, i, j);
            if dist >= field.cutoff {
                continue;
            }
            let sums = field.radii[i as usize] + field.radii[j as usize];
            let unit = (diff.0 / dist, diff.1 / dist);
            let mag = if dist < sums {
                (sums - dist) * 0.5
            } else if field.is_edge(i, j) {
                0.0
            } else {
                0.1 * sums / (dist * dist + 0.1)
            };
            forces[i as usize].0 -= mag * unit.0;
            forces[i as usize].1 -= mag * unit.1;
            forces[j as usize].0 += mag * unit.0;
            forces[j as usize].1 += mag * unit.1;
        }
    }
}

/// Caps each node's force at `temperature`, then applies it (`circle_packing.py:477-480`).
fn cap_and_apply(positions: &mut [(f64, f64)], forces: &[(f64, f64)], temperature: f64) {
    for (p, &f) in positions.iter_mut().zip(forces) {
        let magnitude = libm::hypot(f.0, f.1);
        let (fx, fy) = if magnitude > temperature && magnitude > 0.0 {
            (f.0 * temperature / magnitude, f.1 * temperature / magnitude)
        } else {
            f
        };
        p.0 += fx;
        p.1 += fy;
    }
}

/// The settle pass (`circle_packing.py:487-503`): pure overlap correction, no cooling,
/// stopping as soon as a round pushes nothing.
fn settle(positions: &mut [(f64, f64)], field: &RelaxField, iterations: u32) {
    let rounds = (iterations / 2).max(10);
    for _ in 0..rounds {
        if !settle_round(positions, field) {
            break;
        }
    }
}

/// One Jacobi-batch round: every overlap found from the round's starting positions is
/// shifted, all at once, exactly as SciGraphs' own vectorised pass does.
fn settle_round(positions: &mut [(f64, f64)], field: &RelaxField) -> bool {
    let mut shift = vec![(0.0, 0.0); field.n as usize];
    let mut moved = false;
    for i in 0..field.n {
        for j in (i + 1)..field.n {
            let raw = (
                positions[j as usize].0 - positions[i as usize].0,
                positions[j as usize].1 - positions[i as usize].1,
            );
            let (diff, dist) = separated(raw, i, j);
            let sums = field.radii[i as usize] + field.radii[j as usize];
            if dist >= field.cutoff || dist >= sums {
                continue;
            }
            moved = true;
            let s = (sums - dist) * 0.55 / dist;
            shift[i as usize].0 -= s * diff.0;
            shift[i as usize].1 -= s * diff.1;
            shift[j as usize].0 += s * diff.0;
            shift[j as usize].1 += s * diff.1;
        }
    }
    for (p, s) in positions.iter_mut().zip(&shift) {
        p.0 += s.0;
        p.1 += s.1;
    }
    moved
}
