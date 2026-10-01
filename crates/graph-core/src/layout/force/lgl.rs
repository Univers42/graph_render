//! Large Graph Layout (`layout.force.lgl`), written from `docs/layouts/layout.force.lgl.md`
//! and the paper (Adai et al., J. Mol. Biol. 340(1), 2004) only, per
//! `docs/decisions/layouts-igraph.md`. 2D: a breadth-first tree grows layer by layer, and
//! after each layer the vertices placed so far anneal under attraction along edges and
//! cell-limited repulsion.

mod grow;
#[cfg(test)]
mod tests;

use super::fruchterman_reingold::sqrt;
use super::simple_graph;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::rng::Mulberry32;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// SciGraphs' parameters; `None` takes the binding's default, which depends on `n`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LglParams {
    /// Cooling iterations per layer.
    pub maxit: u32,
    /// Largest displacement per iteration at full temperature; default `n`.
    pub maxdelta: Option<f64>,
    /// Area of the placement disc; default `n^2`.
    pub area: Option<f64>,
    /// Cooling exponent.
    pub coolexp: f64,
    /// Radius term at which repulsion cancels; default `area * n`.
    pub repulserad: Option<f64>,
    /// Cell side and repulsion cut-off; default `area^(1/4)`.
    pub cellsize: Option<f64>,
    /// First vertex of the breadth-first tree; `None` draws one.
    pub root: Option<u32>,
    /// Seeds the root, the scatter and the deep-layer directions (D5).
    pub seed: u32,
}

impl Default for LglParams {
    fn default() -> Self {
        Self {
            maxit: 150,
            maxdelta: None,
            area: None,
            coolexp: 1.5,
            repulserad: None,
            cellsize: None,
            root: None,
            seed: 0,
        }
    }
}

/// Node count past which the per-layer annealing is no longer usable.
pub const LGL_CEILING: u64 = 1_000;

/// Convergence threshold on the largest coordinate change.
const EPSILON: f64 = 1e-5;

/// Parameters resolved against `n`, non-positive values falling back as SciGraphs does.
pub(super) struct Consts {
    pub(super) maxit: u32,
    pub(super) maxdelta: f64,
    pub(super) coolexp: f64,
    pub(super) repulserad: f64,
    pub(super) cellsize: f64,
    pub(super) radius: f64,
    pub(super) ideal: f64,
}

fn positive(value: Option<f64>, default: f64) -> f64 {
    value
        .filter(|v| *v > 0.0 && v.is_finite())
        .unwrap_or(default)
}

fn resolve(n: usize, p: &LglParams) -> Consts {
    let count = n as f64;
    let area = positive(p.area, count * count);
    Consts {
        maxit: p.maxit,
        maxdelta: positive(p.maxdelta, count),
        coolexp: if p.coolexp > 0.0 { p.coolexp } else { 1.5 },
        repulserad: positive(p.repulserad, area * count),
        cellsize: positive(p.cellsize, sqrt(sqrt(area))),
        radius: sqrt(area / core::f64::consts::PI),
        ideal: sqrt(area / count),
    }
}

/// Large Graph Layout stage.
///
/// Ponytail: the repulsion cut-off at the cell size means far clusters never push each
/// other; the result depends on that size. Vertices unreachable from the root keep their
/// random start and take no part in any force (the source warns and does the same), so a
/// disconnected graph is drawn badly, not refused. Three deliberate departures from the
/// spec: layer-1 angles use `2 pi j / size` (the spec's `size - 1` puts the first and last
/// child on one spot), an edge inside one layer is counted once (the spec counts it
/// twice), and convergence reads the largest absolute change (the spec's positive-only
/// change stops early on a step that is negative on both axes). Coordinates never match
/// igraph's: our generator, our cell order.
pub struct Lgl;

impl Stage for Lgl {
    type Params = LglParams;
    const ID: &'static str = "layout.force.lgl";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let mut pos = vec![[0.0; 2]; n];
        if n > 1 {
            let graph = simple_graph(topology);
            let c = resolve(n, params);
            let mut rng = Mulberry32::new(params.seed);
            grow::layout(&graph, &c, (params.root, &mut rng), &mut pos);
        }
        if pos.iter().flatten().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: pos.iter().map(|p| p[0] as f32).collect(),
                y: pos.iter().map(|p| p[1] as f32).collect(),
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}
