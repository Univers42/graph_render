//! Kamada-Kawai (`layout.force.kamada_kawai`), written from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai,
//! Information Processing Letters 31(1), 1989) only, per `docs/decisions/layouts-igraph.md`.
//! 2D, unweighted (every edge has length 1, so all-pairs distances are breadth-first).
//!
//! The Newton descent and the start are [`descent`], shared with
//! [`super::kamada_kawai_3d::KamadaKawai3D`] through a `const D`; this file is the parameters,
//! the graph-derived springs, the 2D stage and the geometry it builds.

#[cfg(test)]
mod tests;

mod descent;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// One node's position, three wide, of which the first `D` are live. See [`descent`]'s module
/// doc for why the width is fixed and the live count is not.
pub(crate) type Axis = [f64; 3];

/// Parameters SciGraphs leaves at igraph's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct KkParams {
    /// Maximum single-vertex moves; `None` means `50 * n`.
    pub maxiter: Option<u32>,
    /// Stop when the largest squared gradient norm falls below this; 0 never stops early.
    pub epsilon: f64,
    /// Spring strength constant; `None` means `n`.
    pub kkconst: Option<f64>,
}

impl Default for KkParams {
    fn default() -> Self {
        Self {
            maxiter: None,
            epsilon: 0.0,
            kkconst: None,
        }
    }
}

/// Node count past which the O(n^2) matrices and 50 n^2 moves are no longer usable.
pub const KK_CEILING: u64 = 2_000;

/// Gradients below this norm are treated as equilibrium: the step is zero.
pub(super) const KK_EPS: f64 = 1e-13;

/// The capability id of the 3D sibling, named here so a caller can reach both dimensions'
/// stages through one import.
pub use super::kamada_kawai_3d::KamadaKawai3D;

/// Kamada-Kawai layout stage. Fully deterministic: the start is a circle, no generator.
///
/// Ponytail: Newton descent on one vertex at a time finds a local minimum of the spring
/// energy, not the global one, so a graph with a folded start can settle folded. An
/// edgeless graph with n >= 2 has no finite distance; igraph divides by zero there, this
/// port takes every distance as 1 instead, so it draws a regular polygon-like cloud.
pub struct KamadaKawai;

impl Stage for KamadaKawai {
    type Params = KkParams;
    const ID: &'static str = "layout.force.kamada_kawai";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let pos = solve::<2>(topology, params)?;
        // `Point` nodes at the columns' own `f32` values, straight `Line` edges, no notes: the
        // planar constructor, so no 2D byte moves by carrying a z column it never had.
        Ok(Geometry::planar(
            NodeGeometry::Point {
                x: narrow(&pos, 0),
                y: narrow(&pos, 1),
            },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

/// The whole stage at `D` columns: the deterministic start, the springs, the descent, and D9's
/// check. Both dimensions differ only in what the caller builds from these columns.
pub(crate) fn solve<const D: usize>(
    topology: &Topology,
    params: &KkParams,
) -> Result<Vec<Axis>, StageError> {
    let n = topology.node_count() as usize;
    let mut pos = descent::start::<D>(n);
    if n > 1 {
        let springs = Springs::new(&simple_graph(topology), n, params);
        descent::descend::<D>(&mut pos, &springs, params);
    }
    if let Some(column) = first_non_finite::<D>(&pos) {
        return Err(StageError::NonFinite { column });
    }
    Ok(pos)
}

/// The column names D9 reports a non-finite value under, in axis order; `D` never exceeds the
/// table, so the lookup is total.
const COLUMN_NAMES: [&str; 3] = ["node.x", "node.y", "node.z"];

/// D9: the first live column holding a value `f64` cannot pin. The axes are walked in order, so
/// the answer does not depend on where the bad value sits among several.
fn first_non_finite<const D: usize>(pos: &[Axis]) -> Option<&'static str> {
    debug_assert!(D <= COLUMN_NAMES.len(), "no name for column {D}");
    for axis in 0..D {
        if pos.iter().any(|p| !p[axis].is_finite()) {
            return Some(COLUMN_NAMES[axis]);
        }
    }
    None
}

/// One live axis of the solved positions, narrowed once.
fn narrow(pos: &[Axis], axis: usize) -> Vec<f32> {
    pos.iter().map(|p| p[axis] as f32).collect()
}

/// Hop distances and the two per-pair constants derived from them.
pub(crate) struct Springs {
    /// Node count; also the width of the distance matrix below.
    pub(crate) n: usize,
    /// `n * n` hop counts, row-major by source.
    dist: Vec<f64>,
    length_per_hop: f64,
    strength: f64,
}

impl Springs {
    fn new(graph: &SimpleGraph, n: usize, params: &KkParams) -> Self {
        let mut dist = all_pairs_hops(graph, n);
        let finite_max = dist
            .iter()
            .copied()
            .filter(|d| d.is_finite())
            .fold(0.0, f64::max);
        let d_max = if finite_max > 0.0 { finite_max } else { 1.0 };
        for d in dist.iter_mut().filter(|d| !d.is_finite() || **d == 0.0) {
            *d = d_max;
        }
        Springs {
            n,
            dist,
            length_per_hop: libm::sqrt(n as f64) / d_max,
            strength: params.kkconst.unwrap_or(n as f64),
        }
    }

    /// Spring `(k, l)` between `i` and `j`.
    pub(crate) fn spring(&self, i: usize, j: usize) -> (f64, f64) {
        let d = self.dist[i * self.n + j];
        (self.strength / (d * d), self.length_per_hop * d)
    }
}

/// Breadth-first hop counts from every vertex; unreachable is infinity.
fn all_pairs_hops(graph: &SimpleGraph, n: usize) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; n * n];
    let mut queue = Vec::with_capacity(n);
    for s in 0..n {
        queue.clear();
        queue.push(s);
        dist[s * n + s] = 0.0;
        let mut head = 0;
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            for &e in graph.rows.row(v as u32) {
                let e = e as usize;
                let u = if graph.lo[e] as usize == v {
                    graph.hi[e]
                } else {
                    graph.lo[e]
                } as usize;
                if dist[s * n + u].is_infinite() {
                    dist[s * n + u] = dist[s * n + v] + 1.0;
                    queue.push(u);
                }
            }
        }
    }
    dist
}
