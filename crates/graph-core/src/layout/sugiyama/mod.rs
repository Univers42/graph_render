//! Sugiyama-style layered DAG drawing (Sugiyama, Tagawa & Toda, "Methods for Visual
//! Understanding of Hierarchical System Structures", 1981). Ported from
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:1-693`; see
//! `docs/decisions/sugiyama-heuristics.md` for the full citation list and every
//! deviation.
//!
//! Pipeline: `acyclic` orients every edge forward, `layering` assigns layers and dummy
//! chains, `ordering` reduces crossings, `coords` assigns X, `routing` builds the
//! geometry.
//!
//! **Two entry points, two sets of axes, one pipeline.** [`run`] is the registered
//! `layout.dag.sugiyama`: X in the priority method's own units (`coords.rs`'s `GAP = 1.0`,
//! uncentred) and Y as `layer * LAYER_SPACING`, which is what the dagre differential
//! measures. [`run_scaled`] is the same six stages with SciGraphs' own per-axis
//! normalisation (`hierarchical.py:679-685`), which is what a byte comparison against
//! `apply_graph_layout` needs. They differ only in that last step.

mod acyclic;
mod coords;
mod layering;
#[cfg(test)]
mod measurement;
mod ordering;
mod routing;
mod scaled;
#[cfg(test)]
mod stages;

pub use scaled::run_scaled;

use super::Geometry;
use crate::index::Topology;
use crate::stage::Stage;
use crate::stage::StageError;
use acyclic::{Acyclic, Arcs};
use coords::Coords;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use layering::{DUMMY_BUDGET, Layering, assign_layers};
use ordering::Ordering;
use routing::{LAYER_SPACING, Routing, edge_paths, node_positions};

/// Cycle breaking through crossing reduction, the three stages [`run`] and
/// `crossings_for` share. Fails only if the layer count does not cover every vertex, which
/// [`layered`]'s own `max() + 1` always does ([`ordering::Ordering::build`]'s guard).
fn layered(topology: &Topology) -> Result<(Acyclic, Layering, Ordering), StageError> {
    let acyclic = Acyclic::of(topology);
    // One sort for the whole layering phase: the arc list is built here and handed down, so
    // `assign_layers`, `budget_plan` and `materialize` read the same list rather than each
    // re-deriving it (the review's finding 2).
    let list = Arcs::new(topology, &acyclic).grouped();
    let layer = assign_layers(&list);
    let layering = Layering::build(&list, &layer, DUMMY_BUDGET);
    let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
    let ordering = Ordering::build(&layering, num_layers)?;
    Ok((acyclic, layering, ordering))
}

/// The layered-DAG stage.
#[derive(Debug, Clone, Copy)]
pub struct Sugiyama;

/// The layered-DAG stage's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SugiyamaParams {
    /// Distance between adjacent layers, along y, or along x when `horizontal`. Finite and
    /// above 0.
    pub layer_spacing: f32,
    /// Lays the layers (rows) out along x, left to right, instead of along y. `false` is the
    /// vertical drawing, bit for bit.
    pub horizontal: bool,
}

impl Default for SugiyamaParams {
    fn default() -> Self {
        Self {
            layer_spacing: LAYER_SPACING,
            horizontal: false,
        }
    }
}

impl Stage for Sugiyama {
    type Params = SugiyamaParams;
    const ID: &'static str = "layout.dag.sugiyama";

    fn run(topology: &Topology, params: &SugiyamaParams) -> Result<Geometry, StageError> {
        let drawing = run(topology, params.layer_spacing)?;
        Ok(if params.horizontal {
            drawing.transposed()
        } else {
            drawing
        })
    }
}

/// Runs the whole pipeline: cycle breaking, layering, crossing reduction, X assignment,
/// then the geometry itself. Fails only on a `layer_spacing` that is not finite and above
/// 0: every `Topology` this crate can build, including the empty one, has a layered drawing.
pub fn run(topology: &Topology, layer_spacing: f32) -> Result<Geometry, StageError> {
    if !(layer_spacing.is_finite() && layer_spacing > 0.0) {
        return Err(StageError::Param {
            name: "layer_spacing",
            rule: "finite and above 0",
        });
    }
    let (acyclic, layering, ordering) = layered(topology)?;
    let coords = Coords::build(&ordering, &layering, topology.node_count());
    let routing = Routing {
        layering: &layering,
        coords: &coords,
        acyclic: &acyclic,
        spacing: layer_spacing,
    };
    let (x, y) = node_positions(&routing, topology.node_count());
    let paths = edge_paths(&routing);
    let mut notes = acyclic.notes;
    notes.extend(layering.notes);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Polyline(paths),
        notes,
    ))
}

/// The crossing count [`run`] would draw `topology` with: the oracle differential's
/// measurement hook (`docs/measurements/phase05-crossings.md`). Test-only: `run` itself
/// never needs it, since the geometry it returns carries no crossing count of its own.
#[cfg(test)]
pub(crate) fn crossings_for(topology: &Topology) -> u64 {
    layered(topology)
        .expect("max() + 1 covers every layer")
        .2
        .crossings
}

/// `dot`'s weighted median (Gansner, Koutsofios, North & Vo 1993): the middle of
/// `values`, `-1.0` for none, the two middle values averaged for an even count except
/// that the pair is weighted by how far each sits from its own end when both gaps are
/// nonzero (`hierarchical.py:444-458`). Shared by [`ordering`] (over integer layer
/// positions) and [`coords`] (over the real-valued X being assigned).
pub(super) fn weighted_median<T>(mut values: Vec<T>) -> f64
where
    T: Copy + Into<f64> + PartialOrd,
{
    if values.is_empty() {
        return -1.0;
    }
    values.sort_by(|a, b| a.partial_cmp(b).expect("no NaN reaches a median"));
    let middle = values.len() / 2;
    if values.len() % 2 == 1 {
        return values[middle].into();
    }
    let (first, last) = (values[0].into(), values[values.len() - 1].into());
    let (below, above) = (values[middle - 1].into(), values[middle].into());
    let (left, right) = (below - first, last - above);
    if left + right == 0.0 {
        // Also the len==2 case: both gaps are 0 against the same two values.
        return (below + above) / 2.0;
    }
    (below * right + above * left) / (left + right)
}

#[cfg(test)]
mod tests;
