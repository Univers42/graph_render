//! `layout.force.spring3d`: [`super::Spring`] at `dim = 3`.
//!
//! SciGraphs' `SPRING_3D` (`networkx_layouts.py:26-34`) is one wrapper over
//! `nx.spring_layout(G, iterations=..., dim=3, scale=..., seed=...)`, and its `SPRING`
//! sibling (`networkx_layouts.py:16-24`) is the same call at `dim=2`. So this is not a
//! second spring layout: it is [`super::Spring`] with the const dimension parameter at 3,
//! over the same [`super::forces::Solver`], the same seed and the same
//! [`super::SpringParams`] — everything the 2D stage declares in its module doc (the
//! departures, the `n >= 500` fork, the chaos) is inherited verbatim and is not repeated
//! here.
//!
//! **What is different, and it is exactly one thing:** the geometry. [`Geometry::in_space`]
//! instead of [`Geometry::planar`], so the snapshot carries the z column and is labelled
//! 0.4 rather than 0.3 (verdict condition 1). Nothing about the solve differs with `D`
//! except that the reductions walk one more column.

use super::{SpringParams, solve};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The 3D layout's capability id, which is also its hash-gate stage. A separate id from
/// `super::ID` because a separate snapshot: the same drawing, labelled 0.4 and carrying a
/// z column, hashes differently from the 2D one and must never be confused with it.
pub const ID_3D: &str = "layout.force.spring3d";

/// `spring_layout` at `dim = 3`: this same algorithm, in space.
///
/// Shares [`SpringParams`] with the 2D stage rather than declaring its own — the reference
/// takes one `iterations` and one `scale` for both (`networkx_layouts.py:26-34`), so a
/// caller comparing the two dimensions moves one value, not two.
pub struct Spring3D;

impl Stage for Spring3D {
    type Params = SpringParams;
    const ID: &'static str = ID_3D;

    /// `solve` at `D = 3`, then a geometry with the z column attached.
    ///
    /// The narrowing is `f64 -> f32` per column, the same cast
    /// `crate::layout::coords::point_geometry` makes for the 2D stage, so the x and y of a
    /// 3D node are the `f32` of the same `f64` — the column is carried, not recomputed.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let field = solve::<3>(topology, params)?;
        let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
        Ok(Geometry::in_space(
            NodeGeometry::Point {
                x: narrow(&field.c[0]),
                y: narrow(&field.c[1]),
            },
            EdgeGeometry::Line,
            Vec::new(),
            narrow(&field.c[2]),
        ))
    }
}

#[cfg(test)]
#[path = "spring3d/snapshots.rs"]
mod snapshots;
#[cfg(test)]
#[path = "spring3d/tests.rs"]
mod tests;
