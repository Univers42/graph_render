//! Deterministic initial positions for [`super::pack`]'s force pass: a port of
//! networkx 3.6's dense `_fruchterman_reingold` (`drawing/layout.py:660-720`).
//!
//! **Two starts, one per caller, never a blend.** SciGraphs seeds this pass with
//! `nx.spring_layout`'s own uniform-random positions, `seed.rand(nnodes, dim)`
//! (`circle_packing.py:428`), which at an `int` seed is numpy's legacy `RandomState` — so
//! [`start_positions`] at [`Some`] draws exactly that, bit for bit, and that is what the
//! SciGraphs conformance arm passes. At [`None`] the golden-angle spiral stands in:
//! `sqrt((i+1)/n)` out at `i * GOLDEN_ANGLE`, normalised into the unit disk (the spiral
//! `src/core/layout/forceLayout.ts:63` already uses for the interactive force layout; that
//! one is a pixel-space spiral around a viewport centre, this one has no viewport). The
//! registered `layout.packing.circle` keeps `None`, because the spiral is a heuristic whose
//! coordinates are hashed, and the row that needs the reference's numbers is measured by
//! one arm that asks for them.
//!
//! Ponytail: the spiral is a *heuristic start*, and heuristics get a marker. Failing input:
//! a graph whose packing is decided by where it started — but the relaxation that follows
//! finds its own equilibrium regardless of where it started, so the direction of the error
//! is a different (never a wrong) starting layout, and it is SciGraphs' own numbers that
//! are wanted whenever they can be had. Escape hatch: the `seed` argument is the hatch; the
//! whole path is already flagged by note code 3.

/// The golden angle in radians, `src/core/layout/forceLayout.ts`'s `GOLDEN_ANGLE`.
pub(super) const GOLDEN_ANGLE: f64 = 2.399_963_229_728_653;

/// Node `i`'s spiral seed: `sqrt((i + 1) / n)` out at angle `i * GOLDEN_ANGLE`, so `n`
/// points fill the unit disk with no two at the same radius.
///
/// **`None` only.** This is the registered `layout.packing.circle`'s own start, and the
/// SciGraphs arm is [`seeded`] — the reference's `RandomState` stream, not this spiral.
pub(super) fn seed_positions(n: u32) -> Vec<(f64, f64)> {
    let total = f64::from(n.max(1));
    (0..n)
        .map(|i| {
            let radius = f64::sqrt(f64::from(i + 1) / total);
            let angle = f64::from(i) * GOLDEN_ANGLE;
            (radius * libm::cos(angle), radius * libm::sin(angle))
        })
        .collect()
}

/// The SciGraphs arm: `np.random.RandomState(seed).rand(n, 2)` (`drawing/layout.py:610`),
/// row-major — node `i` takes draws `2i` and `2i + 1` as `x` and `y`, which is the fill
/// order of the `n x dim` array `spring_layout` hands `_fruchterman_reingold`.
///
/// networkx builds that generator from an `int` seed (`utils/misc.py:290-291`) and
/// SciGraphs passes `seed=get_layout_seed()` (`circle_packing.py:428`), so this is the
/// reference's own doubles: two `u32` words each, and no scaling — `dom_size` is 1 and
/// `center` is 0 at `circle_packing.py:429`.
///
/// Caveat: the **dense** arm only. At `n >= 500` `spring_layout` builds `A` with
/// `dtype="f"` (`layout.py:629`) and casts `pos` to it (`layout.py:672`), so the
/// reference's start is float32 there and half its bits are gone before the first force. No
/// conformance fixture reaches 500 nodes and this fallback is a non-planar-graph path, so
/// nothing here is measured against that fork.
fn seeded(n: u32, seed: u32) -> Vec<(f64, f64)> {
    let mut stream = crate::rng::Mt19937::new(seed);
    (0..n)
        .map(|_| (stream.next_f64(), stream.next_f64()))
        .collect()
}

/// The start [`fruchterman_reingold`] relaxes from: the reference's stream at
/// [`Some`]`, the golden-angle spiral at `None`. One arm or the other, never a blend —
/// a start drawn from one generator and continued from the other is neither.
pub(super) fn start_positions(n: u32, seed: Option<u32>) -> Vec<(f64, f64)> {
    match seed {
        Some(s) => seeded(n, s),
        None => seed_positions(n),
    }
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
        f64::sqrt(moved_sq)
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
/// so `k = sqrt(1 / n)`, started from [`start_positions`] at `seed` — the reference's
/// `RandomState` run, or the golden-angle spiral when there is no seed.
pub(super) fn fruchterman_reingold(
    n: u32,
    edges: &[(u32, u32)],
    iterations: u32,
    seed: Option<u32>,
) -> Vec<(f64, f64)> {
    let adjacency = adjacency_matrix(n, edges);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(1.0 / f64::from(n.max(1))),
    };
    let mut pos = start_positions(n, seed);
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
