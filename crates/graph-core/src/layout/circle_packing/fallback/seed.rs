//! Deterministic initial positions for [`super::pack`]'s force pass: a port of
//! networkx 3.6's dense `_fruchterman_reingold` (`drawing/layout.py:660-720`), seeded by
//! a fixed spiral instead of `numpy`'s RNG.
//!
//! Ponytail: SciGraphs seeds this fallback with `nx.spring_layout`'s own uniform-random
//! positions (`seed.rand(nnodes, dim)`); graph-core has no RNG and no wall clock
//! (`prompts/REFERENCES.md` house limits), so this substitutes the golden-angle spiral
//! `src/core/layout/forceLayout.ts:63` already uses to seed the interactive force
//! layout, normalised into the unit disk here (that one is a pixel-space spiral around a
//! viewport centre; this one has no viewport, so it is scaled to `sqrt((i+1)/n)` instead
//! of a fixed pixel radius). Failing input: none — every node gets a distinct point on
//! the spiral, never a collision. Direction: a different starting layout than SciGraphs
//! would draw for the same graph, never a wrong one — the relaxation that follows finds
//! its own equilibrium regardless of where it started. Escape hatch: none needed; this
//! whole path is already flagged by note code 3.

/// The golden angle in radians, `src/core/layout/forceLayout.ts`'s `GOLDEN_ANGLE`.
pub(super) const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// Node `i`'s spiral seed: `sqrt((i + 1) / n)` out at angle `i * GOLDEN_ANGLE`, so `n`
/// points fill the unit disk with no two at the same radius.
pub(super) fn seed_positions(n: u32) -> Vec<(f64, f64)> {
    let total = f64::from(n.max(1));
    (0..n)
        .map(|i| {
            let radius = libm::sqrt(f64::from(i + 1) / total);
            let angle = f64::from(i) * GOLDEN_ANGLE;
            (radius * libm::cos(angle), radius * libm::sin(angle))
        })
        .collect()
}

/// The dense adjacency SciGraphs builds with `nx.to_numpy_array` (unweighted here, so
/// `A[i][j]` is the multiplicity of the `(i, j)` edge; a self-loop's own diagonal entry
/// is never read since every pairwise force term skips `i == j`).
fn adjacency_matrix(n: u32, edges: &[(u32, u32)]) -> Vec<f64> {
    let n = n as usize;
    let mut a = vec![0.0; n * n];
    for &(u, v) in edges {
        let (u, v) = (u as usize, v as usize);
        a[u * n + v] += 1.0;
        a[v * n + u] += 1.0;
    }
    a
}

/// The dense adjacency, node count and optimal distance a Fruchterman–Reingold step
/// needs, bundled so [`fruchterman_reingold`] and [`FrField::step`] stay under the house
/// parameter cap.
struct FrField<'a> {
    adjacency: &'a [f64],
    n: u32,
    k: f64,
}

impl FrField<'_> {
    /// One node's net displacement force from every other node
    /// (`drawing/layout.py:697-705`): `k^2 / dist^2` repulsion from everyone, `A[i][j] *
    /// dist / k` attraction along an edge's weight, distance floored at `0.01`.
    fn displacement(&self, pos: &[(f64, f64)], i: usize) -> (f64, f64) {
        let n = self.n as usize;
        let (mut fx, mut fy) = (0.0, 0.0);
        for j in 0..n {
            if j == i {
                continue;
            }
            let (dx, dy) = (pos[i].0 - pos[j].0, pos[i].1 - pos[j].1);
            let dist = libm::hypot(dx, dy).max(0.01);
            let factor =
                self.k * self.k / (dist * dist) - self.adjacency[i * n + j] * dist / self.k;
            fx += dx * factor;
            fy += dy * factor;
        }
        (fx, fy)
    }

    /// One temperature-capped step for every node, returning the Frobenius norm of the
    /// whole move (`drawing/layout.py:707-719`).
    ///
    /// Jacobi, as the reference is: it computes the whole `delta_pos` matrix from `pos`'s
    /// starting state and only then evaluates `pos += delta_pos` (`layout.py:711`, `:715`),
    /// so a node never reads a neighbour's move from the same round. Computing each
    /// displacement from the live array instead would be Gauss–Seidel, a different
    /// iteration with a different fixed point and different output — the reduction is in
    /// the same ascending-`j` order either way (D3), so the only difference is the
    /// snapshot the terms are read from.
    fn step(&self, pos: &mut [(f64, f64)], t: f64) -> f64 {
        let start: Vec<(f64, f64)> = pos.to_vec();
        let mut delta_pos = Vec::with_capacity(start.len());
        let mut moved_sq = 0.0;
        for i in 0..start.len() {
            let d = self.displacement(&start, i);
            let len = libm::hypot(d.0, d.1).max(0.01);
            let (dx, dy) = (d.0 * t / len, d.1 * t / len);
            delta_pos.push((dx, dy));
            moved_sq += dx * dx + dy * dy;
        }
        for (p, &(dx, dy)) in pos.iter_mut().zip(&delta_pos) {
            p.0 += dx;
            p.1 += dy;
        }
        libm::sqrt(moved_sq)
    }
}

/// `(max - min)` over each axis of `pos`, times `0.1` — the reference's own initial
/// "temperature" (`drawing/layout.py:685-687`).
fn initial_temperature(pos: &[(f64, f64)]) -> f64 {
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for &(x, y) in pos {
        min_x = min_x.min(x);
        max_x = max_x.max(x);
        min_y = min_y.min(y);
        max_y = max_y.max(y);
    }
    (max_x - min_x).max(max_y - min_y) * 0.1
}

/// Below this per-node average move, the walk has settled (`spring_layout`'s own
/// `threshold` default, `drawing/layout.py:509, :662`).
const THRESHOLD: f64 = 1e-4;

/// networkx 3.6's dense `_fruchterman_reingold` (`drawing/layout.py:660-720`), `k = None`
/// so `k = sqrt(1 / n)`, seeded by [`seed_positions`] instead of `seed.rand`.
pub(super) fn fruchterman_reingold(
    n: u32,
    edges: &[(u32, u32)],
    iterations: u32,
) -> Vec<(f64, f64)> {
    let adjacency = adjacency_matrix(n, edges);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: libm::sqrt(1.0 / f64::from(n.max(1))),
    };
    let mut pos = seed_positions(n);
    let mut t = initial_temperature(&pos);
    let dt = t / f64::from(iterations + 1);
    for _ in 0..iterations {
        let moved = field.step(&mut pos, t);
        t -= dt;
        if moved / f64::from(n.max(1)) < THRESHOLD {
            break;
        }
    }
    pos
}

/// `rescale_layout` (`drawing/layout.py:1882-1924`): mean-centre, then scale so the
/// largest-magnitude coordinate on either axis becomes `scale`.
pub(super) fn rescale_to(positions: &mut [(f64, f64)], scale: f64) {
    super::super::geometry::center(positions);
    let max = positions
        .iter()
        .fold(0.0_f64, |m, &(x, y)| m.max(x.abs()).max(y.abs()));
    if max > 0.0 {
        for p in positions.iter_mut() {
            p.0 *= scale / max;
            p.1 *= scale / max;
        }
    }
}

#[cfg(test)]
mod tests;
