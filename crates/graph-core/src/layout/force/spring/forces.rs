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
//!
//! **One kernel at `D` columns, not one kernel per dimension.** `D` is a const parameter and
//! every column is walked in axis order `x`, then `y`, then `z`, so `D = 2` performs exactly
//! the operations, in exactly the order, that the two-column version did: every reduction
//! below accumulates a fixed sum of squares *per node, per column*, in that order, and each
//! column's total is independent of the others. `layout.force.spring3d` is this file at
//! `D = 3`, not a second copy of it — SciGraphs' `SPRING_3D`
//! (`networkx_layouts.py:26-34`) is the same `nx.spring_layout` call as `SPRING`
//! (`networkx_layouts.py:16-24`) with the `dim` literal changed and nothing else.

use super::{MIN_DISTANCE, MIN_LENGTH, SpringParams};
use crate::layout::force::SimpleGraph;

/// The column names D9 reports a non-finite value under, in axis order. `D` never exceeds
/// the table, so the lookup is total.
const COLUMN_NAMES: [&str; 3] = ["node.x", "node.y", "node.z"];

/// One step's worth of positions, `f64` throughout, narrowed to `f32` once at the end.
///
/// `D` columns per node: `2` for [`super::Spring`], `3` for [`super::spring3d::Spring3D`].
pub(super) struct Field<const D: usize> {
    /// Per-node x, then y, then z — axis-major, so a column is a contiguous slice and the
    /// reductions below walk one at a time in ascending node index.
    pub(super) c: [Vec<f64>; D],
}

impl<const D: usize> Field<D> {
    /// `n` nodes at the origin.
    pub(super) fn zeros(n: u32) -> Self {
        Field {
            c: core::array::from_fn(|_| vec![0.0; n as usize]),
        }
    }
}

/// The dense solve over one graph: the adjacency, and `k`, the optimal distance.
pub(super) struct Solver<'a, const D: usize> {
    graph: &'a SimpleGraph,
    n: u32,
    k: f64,
}

impl<'a, const D: usize> Solver<'a, D> {
    /// `k = sqrt(1 / n)` at `layout.py:701-702`. `nnodes` is fixed for the whole solve, so
    /// this is computed once and the temperature schedule below is the only per-step
    /// state — which is what makes the loop a pure function of `(cur, t)`.
    pub(super) fn new(graph: &'a SimpleGraph, n: u32) -> Self {
        Solver {
            graph,
            n,
            k: f64::sqrt(1.0 / f64::from(n)),
        }
    }

    /// The whole iteration: `params.iterations` gathers, stopping as soon as a step's
    /// total movement falls under networkx's own `threshold` (`layout.py:725-726`).
    pub(super) fn settle(self, start: Field<D>, params: SpringParams) -> Field<D> {
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

    /// The opening temperature, `layout.py:705-706`: a tenth of the largest coordinate span
    /// over the `D` columns, so the first step is bounded by the domain the start occupies.
    fn opening(&self, cur: &Field<D>) -> f64 {
        let mut widest = span(&cur.c[0]);
        for axis in 1..D {
            widest = widest.max(span(&cur.c[axis]));
        }
        widest * 0.1
    }

    /// One gather: every `out[i]` from the start-of-step `cur` alone (D10).
    fn gather(&self, t: f64, cur: &Field<D>, out: &mut Field<D>) {
        for i in 0..self.n as usize {
            let delta = self.displacement(i as u32, cur);
            let length = f64::sqrt(squared(&delta)).max(MIN_LENGTH);
            for (axis, column) in out.c.iter_mut().enumerate() {
                column[i] = cur.c[axis][i] + delta[axis] * (t / length);
            }
        }
    }

    /// Node `i`'s displacement: the `k*k/d^2 - A*d/k` force of `layout.py:715-719`, with
    /// the repulsion summed over every other node and the attraction over `i`'s own
    /// edges. `j == i` is skipped: its `delta` is zero, so it contributes nothing however
    /// the two terms are bounded.
    fn displacement(&self, i: u32, cur: &Field<D>) -> [f64; D] {
        let mut delta = [0.0; D];
        for j in 0..self.n {
            if j == i {
                continue;
            }
            let ex = self.separation(i, j, cur);
            let d = clipped(&ex);
            let repulsion = self.k * self.k / (d * d);
            for axis in 0..D {
                delta[axis] += ex[axis] * repulsion;
            }
        }
        for edge in self.graph.rows.row(i) {
            let j = self.graph.other(*edge, i);
            let ex = self.separation(i, j, cur);
            let attraction = clipped(&ex) / self.k;
            for axis in 0..D {
                delta[axis] -= ex[axis] * attraction;
            }
        }
        delta
    }

    /// `pos[i] - pos[j]`, one cell of the `delta` matrix at `layout.py:711`, axis by axis.
    fn separation(&self, i: u32, j: u32, cur: &Field<D>) -> [f64; D] {
        let (i, j) = (i as usize, j as usize);
        let mut delta = [0.0; D];
        for (axis, column) in cur.c.iter().enumerate() {
            delta[axis] = column[i] - column[j];
        }
        delta
    }

    /// `np.linalg.norm(delta_pos) / nnodes` — the early-exit numerator of
    /// `layout.py:725`, summed in ascending index order (D3). The squared length of one
    /// node's movement is finished before it joins the running total, which is the order
    /// the two-column version accumulated in.
    fn travelled(&self, cur: &Field<D>, out: &Field<D>) -> f64 {
        let mut sum = 0.0;
        for i in 0..self.n as usize {
            let mut moved = [0.0; D];
            for (axis, column) in out.c.iter().enumerate() {
                moved[axis] = column[i] - cur.c[axis][i];
            }
            sum += squared(&moved);
        }
        f64::sqrt(sum)
    }
}

/// `np.linalg.norm(delta)` over the `D` columns: the squared length finished inside the
/// axis loop, so a caller adds one total per node and the rounding is axis-order only.
///
/// For `D = 2` this is `dx * dx + dy * dy` — `0.0 + dx * dx` is `dx * dx`, and no product of
/// two `f64`s is a negative zero, so the leading zero cannot move a bit.
fn squared<const D: usize>(delta: &[f64; D]) -> f64 {
    let mut sum = 0.0;
    for &v in delta {
        sum += v * v;
    }
    sum
}

/// `np.clip(distance, 0.01, None)` (`layout.py:713`): the reference's minimum separation,
/// which is what keeps an exact coincidence from dividing by zero. Every `sqrt` in the
/// port is libm's, so it is bit-identical on every target (D1, D2).
fn clipped<const D: usize>(delta: &[f64; D]) -> f64 {
    f64::sqrt(squared(delta)).max(MIN_DISTANCE)
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

/// D9: the first column holding a value `f64` cannot pin, named as the snapshot names its
/// columns — `node.z` for the third, which is the column only `D = 3` has. The axes are
/// walked in order, so the answer does not depend on where the bad value sits among
/// several, and `D` is bounded by [`COLUMN_NAMES`].
pub(super) fn first_non_finite<const D: usize>(field: &Field<D>) -> Option<&'static str> {
    debug_assert!(D <= COLUMN_NAMES.len(), "no name for column {D}");
    for (axis, column) in field.c.iter().enumerate() {
        if column.iter().any(|v| !v.is_finite()) {
            return Some(COLUMN_NAMES[axis]);
        }
    }
    None
}
