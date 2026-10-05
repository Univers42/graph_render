//! `layout.dag.lanes`: one row per vertex in a topological order of the directed edges, each
//! line of descent kept in a lane, a lane reused once it is free, and every edge a polyline of
//! at most two interior points. No dummy vertices and no crossing reduction, so the cost is
//! O((n + m) log n) with no budget to run out of.
//! Spec: `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`.

mod assign;
mod geometry;
mod rows;
#[cfg(test)]
mod tests;

use super::Geometry;
use crate::index::Topology;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The registry id.
pub const ID: &str = "layout.dag.lanes";

/// The first row whose half-row bend is not representable in `f32`: `(r as f32) + 0.5` is
/// exact only while `r < 2^23`, so from here a bend lands on an adjacent vertex row and the
/// polyline runs along a row instead of between two. `docs/decisions/dag-lanes.md`
/// condition 3, refusal form (a).
pub const MAX_ROWS: u32 = 1 << 23;

/// The lanes stage.
#[derive(Debug, Clone, Copy)]
pub struct Lanes;

/// The lanes stage's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LanesParams {
    /// X distance between adjacent lanes. Finite and above 0.
    pub lane_spacing: f32,
    /// Y distance between adjacent rows. Finite and above 0.
    pub row_spacing: f32,
}

impl Default for LanesParams {
    fn default() -> Self {
        Self { lane_spacing: 1.0, row_spacing: 1.0 }
    }
}

impl Stage for Lanes {
    type Params = LanesParams;
    const ID: &'static str = ID;

    fn run(topology: &Topology, params: &LanesParams) -> Result<Geometry, StageError> {
        run(topology, params)
    }
}

/// Rows, then lanes, then the geometry. Fails on a spacing that is not finite and above 0,
/// and on a graph too tall for `f32`'s half-row bend: every `Topology` below that row has a
/// lanes drawing.
pub fn run(topology: &Topology, params: &LanesParams) -> Result<Geometry, StageError> {
    legal("lane_spacing", params.lane_spacing)?;
    legal("row_spacing", params.row_spacing)?;
    rows_fit(topology.node_count())?;
    let rows = rows::Rows::of(topology);
    let drawing = assign::Drawing::of(topology, &rows);
    let (x, y) = geometry::positions(&drawing, &rows, params);
    let paths = geometry::paths(&drawing, topology, &rows, params);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Polyline(paths),
        rows.notes(topology),
    ))
}

/// Whether `n` rows still draw: the half-row bend is exact in `f32` below [`MAX_ROWS`] and
/// is not at or above it, so a taller graph is refused rather than drawn wrong.
pub fn rows_fit(n: u32) -> Result<(), StageError> {
    if n < MAX_ROWS {
        return Ok(());
    }
    Err(StageError::Param { name: "nodes", rule: "below 2^23 rows: the half-row bend is not representable in f32" })
}

fn legal(name: &'static str, value: f32) -> Result<(), StageError> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(StageError::Param { name, rule: "finite and above 0" })
}
