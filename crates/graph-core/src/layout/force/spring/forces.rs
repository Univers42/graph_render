//! The iteration itself: one dense Fruchterman–Reingold pass per step, in gather form.
//!
//! Ported from networkx 3.6 `_fruchterman_reingold`
//! (`networkx/drawing/layout.py:660-727`) — the `method="force"` branch, which is what
//! `spring_layout` runs for a graph under 500 nodes (`layout.py:140-141`). One step is
//! **gather form (D10)**: `out[i]` reads only the start-of-step `cur` and writes only
//! `out[i]`, so the pass is a Jacobi step and two threads cannot see each other's writes.
//! The reference is also a Jacobi step — it computes the whole `delta_pos` before touching
//! `pos` (`layout.py:722`) — so this is the same arithmetic, and it is the only reason a
//! threaded arm can agree with the scalar one at all.
//!
//! **Fixed order (D3).** Both reductions run in ascending dense index: the repulsion over
//! `j = 0..n`, the attraction over the node's own CSR row in edge order. The reference
//! sums one fused pass over `j` with `A[i, j]` as a dense lookup; splitting the attraction
//! out is the same sum with a different rounding, and it is what lets the port read the
//! graph as a CSR instead of materialising an `n x n` matrix. The port is therefore not
//! bit-faithful to the reference at any budget — stated rather than hidden, and it costs
//! nothing here: the layout is chaotic, so the differential is a stress ratio and not a
//! coordinate gap (module doc).

use super::{MIN_DISTANCE, MIN_LENGTH, SpringParams};
use crate::layout::force::SimpleGraph;

/// One step's worth of positions, `f64` throughout, narrowed to `f32` once at the end.
pub(super) struct Field {
    /// Per-node x.
    pub(super) x: Vec<f64>,
    /// Per-node y.
    pub(super) y: Vec<f64>,
}

impl Field {
    /// `n` nodes at the origin.
    pub(super) fn zeros(n: u32) -> Self {
        Field {
            x: vec![0.0; n as usize],
            y: vec![0.0; n as usize],
        }
    }
}

/// The dense solve over one graph: the adjacency, and `k`, the optimal distance.
pub(super) struct Solver<'a> {
    graph: &'a SimpleGraph,
    n: u32,
    k: f64,
}

impl<'a> Solver<'a> {
    /// `k = sqrt(1 / n)` at `layout.py:701-702`. `nnodes` is fixed for the whole solve, so
    /// this is computed once and the temperature schedule below is the only per-step
    /// state — which is what makes the loop a pure function of `(cur, t)`.
    pub(super) fn new(graph: &'a SimpleGraph, n: u32) -> Self {
        Solver {
            graph,
            n,
            k: libm::sqrt(1.0 / f64::from(n)),
        }
    }

    /// The whole iteration: `params.iterations` gathers, stopping as soon as a step's
    /// total movement falls under networkx's own `threshold` (`layout.py:725-726`).
    pub(super) fn settle(self, start: Field, params: SpringParams) -> Field {
        let mut cur = start;
        let mut out = Field::zeros(self.n);
        let mut t = self.opening(&cur);
        let dt = t / f64::from(params.iterations + 1);
        for _ in 0..params.iterations {
            self.gather(t, &cur, &mut out);
            let moved = self.travelled(&cur, &out) / f64::from(self.n);
            std::mem::swap(&mut cur, &mut out);
            t -= dt;
            if moved < params.threshold {
                break;
            }
        }
        cur
    }

    /// The opening temperature, `layout.py:705-706`: a tenth of the larger of the two
    /// coordinate spans, so the first step is bounded by the domain the start occupies.
    fn opening(&self, cur: &Field) -> f64 {
        span(&cur.x).max(span(&cur.y)) * 0.1
    }

    /// One gather: every `out[i]` from the start-of-step `cur` alone (D10).
    fn gather(&self, t: f64, cur: &Field, out: &mut Field) {
        for i in 0..self.n as usize {
            let (dx, dy) = self.displacement(i as u32, cur);
            let length = libm::sqrt(dx * dx + dy * dy).max(MIN_LENGTH);
            out.x[i] = cur.x[i] + dx * (t / length);
            out.y[i] = cur.y[i] + dy * (t / length);
        }
    }

    /// Node `i`'s displacement: the `k*k/d^2 - A*d/k` force of `layout.py:715-719`, with
    /// the repulsion summed over every other node and the attraction over `i`'s own
    /// edges. `j == i` is skipped: its `delta` is zero, so it contributes nothing however
    /// the two terms are bounded.
    fn displacement(&self, i: u32, cur: &Field) -> (f64, f64) {
        let (mut dx, mut dy) = (0.0, 0.0);
        for j in 0..self.n {
            if j == i {
                continue;
            }
            let (ex, ey) = self.separation(i, j, cur);
            let d = clipped(ex, ey);
            let repulsion = self.k * self.k / (d * d);
            dx += ex * repulsion;
            dy += ey * repulsion;
        }
        for edge in self.graph.rows.row(i) {
            let j = self.graph.other(*edge, i);
            let (ex, ey) = self.separation(i, j, cur);
            let attraction = clipped(ex, ey) / self.k;
            dx -= ex * attraction;
            dy -= ey * attraction;
        }
        (dx, dy)
    }

    /// `pos[i] - pos[j]`, one cell of the `delta` matrix at `layout.py:711`.
    fn separation(&self, i: u32, j: u32, cur: &Field) -> (f64, f64) {
        (
            cur.x[i as usize] - cur.x[j as usize],
            cur.y[i as usize] - cur.y[j as usize],
        )
    }

    /// `np.linalg.norm(delta_pos) / nnodes` — the early-exit numerator of
    /// `layout.py:725`, summed in ascending index order (D3).
    fn travelled(&self, cur: &Field, out: &Field) -> f64 {
        let mut sum = 0.0;
        for i in 0..self.n as usize {
            let (dx, dy) = (out.x[i] - cur.x[i], out.y[i] - cur.y[i]);
            sum += dx * dx + dy * dy;
        }
        libm::sqrt(sum)
    }
}

/// `np.clip(distance, 0.01, None)` (`layout.py:713`): the reference's minimum separation,
/// which is what keeps an exact coincidence from dividing by zero. Every `sqrt` in the
/// port is libm's, so it is bit-identical on every target (D1, D2).
fn clipped(ex: f64, ey: f64) -> f64 {
    libm::sqrt(ex * ex + ey * ey).max(MIN_DISTANCE)
}

/// The largest coordinate minus the smallest, for the opening temperature. Ascending
/// index, two independent extremes, so the order cannot reach the result (D3).
fn span(column: &[f64]) -> f64 {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for &v in column {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    hi - lo
}
