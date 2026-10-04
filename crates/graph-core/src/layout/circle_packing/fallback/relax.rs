//! The fallback's relaxation itself: the edge springs, the overlap pass, the temperature
//! cap and the settle rounds SciGraphs runs after the seed
//! (`circle_packing.py:437-513`). Its own Ponytails live on the two functions that carry
//! them; everything here is SciGraphs' arithmetic, ported term for term.

use super::separated;

/// `relax`'s own tunables, bundled so it stays under the parameter cap.
pub(super) struct RelaxParams {
    pub(super) iterations: u32,
    pub(super) scale: f64,
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
/// pure-overlap settle pass (`circle_packing.py:437-513`). Neither goal is guaranteed,
/// unlike the exact path.
pub(super) fn relax(
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
        let mut forces = edge_pull(&positions, &field, edges);
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

/// Every node's spring pull toward its incident edges' target tangency
/// (`circle_packing.py:444-454`), as a gather (D10): node `i` sums its own edges in
/// edge-list order and writes only `out[i]`. `v`'s term is the negation of the very same
/// product `u`'s is, so this is bit-identical to the `forces[u] += / forces[v] -=` scatter
/// it replaces — proved bit for bit by
/// `tests::the_edge_pull_gather_is_bit_identical_to_the_scatter_over_seeded_cases`.
fn edge_pull(
    positions: &[(f64, f64)],
    field: &RelaxField,
    edges: &[(u32, u32)],
) -> Vec<(f64, f64)> {
    let n = field.n as usize;
    let incidence = incidence_lists(edges, n);
    let mut forces = vec![(0.0, 0.0); n];
    for (i, incident) in incidence.iter().enumerate() {
        let (mut fx, mut fy) = (0.0, 0.0);
        for &e in incident {
            let &(u, v) = &edges[e];
            let (dx, dy) = spring(positions, field, (u, v));
            if u as usize == i {
                fx += dx;
                fy += dy;
            } else {
                fx -= dx;
                fy -= dy;
            }
        }
        forces[i] = (fx, fy);
    }
    forces
}

/// One edge's pull: the `(x, y)` added into `u`'s force, whose negation is `v`'s.
fn spring(positions: &[(f64, f64)], field: &RelaxField, edge: (u32, u32)) -> (f64, f64) {
    let (u, v) = edge;
    let target = field.radii[u as usize] + field.radii[v as usize];
    let raw = (
        positions[v as usize].0 - positions[u as usize].0,
        positions[v as usize].1 - positions[u as usize].1,
    );
    let (diff, dist) = separated(raw, u, v);
    let pull = (dist - target) * 0.3 / dist;
    (pull * diff.0, pull * diff.1)
}

/// `incidence[i]`: every index into `edges` incident to node `i`, ascending — the order
/// the edge loop visited them in, and the order the gather must sum them in to stay
/// bit-identical. A self-loop (which [`crate::layout::circle_packing::simple_pairs`]
/// filters out, but a caller may pass) appears twice, so its `+` then `-` on the same
/// node survives the rewrite.
fn incidence_lists(edges: &[(u32, u32)], n: usize) -> Vec<Vec<usize>> {
    let mut incidence = vec![Vec::new(); n];
    for (i, &(u, v)) in edges.iter().enumerate() {
        incidence[u as usize].push(i);
        incidence[v as usize].push(i);
    }
    incidence
}

/// Every close pair pushed apart if it overlaps, or weakly repelled if it neither
/// overlaps nor shares a graph edge (`circle_packing.py:456-480`).
///
/// Ponytail: **this is the one scatter left in the module, on purpose.** It is the
/// `O(n^2)` all-pairs scan, so each pair is visited once and both endpoints are updated
/// in place; a gather would need a full list of close pairs per node, which is the same
/// `O(n^2)` enumeration with an `O(n^2)`-sized incidence list built and carried to do it,
/// for no reduction in work. So the result is bit-exact only in this scalar ascending-
/// `(i, j)` order, and this function is not eligible for the Phase 11 compute tiers,
/// whose value is bit-identity with the scalar build. `settle_round` below is in the same
/// position. Direction: a reordering here changes the packing's last bits, never its
/// correctness — the whole path is already flagged approximate (note code 3) and its own
/// Ponytail says neither tangency nor non-overlap is guaranteed. Escape hatch: none
/// needed; Phase 9 owns replacing this `O(n^2)` fallback, and its replacement must be
/// gather form (D10), as `docs/decisions/planarity-fallback.md`'s "D10 exception" section
/// records.
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

/// Caps each node's force at `temperature`, then applies it (`circle_packing.py:482-486`).
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

/// The settle pass (`circle_packing.py:493-513`): pure overlap correction, no cooling,
/// stopping as soon as a round pushes nothing.
fn settle(positions: &mut [(f64, f64)], field: &RelaxField, iterations: u32) {
    for _ in 0..settle_rounds(iterations) {
        if !settle_round(positions, field) {
            break;
        }
    }
}

/// The settle pass's round cap, SciGraphs' own `max(int(iterations) // 2, 10)`
/// (`circle_packing.py:493`): half the relaxation budget, floored at 10 so even the
/// smallest budget still gets a real pass.
fn settle_rounds(iterations: u32) -> u32 {
    (iterations / 2).max(10)
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

#[cfg(test)]
mod tests;
