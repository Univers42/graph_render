//! Stand-in registered layouts for `stages.rs`'s tests. Split into its own file because
//! they are pure fixtures: three `Capability` rows that write a distinct constant into
//! `x[0]`, so a test can tell one registered layout's hashed bytes from another's.
//!
//! Why not three copies of the grid: a copy's `run` *is* the grid's, so it produces
//! identical bytes, and a native arm that hashed the grid for every registered layout
//! would pass a test written with copies. The marker is what makes that substitution
//! observable.

use super::super::super::LAYOUT;
use graph_contract::geometry::NodeGeometry;
use graph_core::registry::Capability;
use graph_core::{Geometry, StageError, Topology};

/// The ids of an already-run stage list.
pub fn ids_of(stages: &[(&'static str, Vec<u8>)]) -> Vec<&'static str> {
    stages.iter().map(|(id, _)| *id).collect()
}

/// Two layouts whose `run` marks `x[0]` with a constant of their own.
pub fn marked_layouts() -> [Capability; 2] {
    let grid = grid();
    [
        Capability {
            id: "layout.grid.left",
            run: marked_left,
            meta: grid.meta,
        },
        Capability {
            id: "layout.grid.right",
            run: marked_right,
            meta: grid.meta,
        },
    ]
}

/// The grid, plus both marked layouts: three registered layouts, three layout stages.
pub fn crowded() -> [Capability; 3] {
    let [left, right] = marked_layouts();
    [grid(), left, right]
}

fn grid() -> Capability {
    *graph_core::registry::find(LAYOUT).expect("the grid is registered")
}

fn mark(mut geometry: Result<Geometry, StageError>, value: f32) -> Result<Geometry, StageError> {
    if let Ok(ref mut geometry) = geometry
        && let NodeGeometry::Point { x, .. } = &mut geometry.nodes
    {
        x[0] = value;
    }
    geometry
}

fn marked_left(topology: &Topology) -> Result<Geometry, StageError> {
    mark((grid().run)(topology), 111.0)
}

fn marked_right(topology: &Topology) -> Result<Geometry, StageError> {
    mark((grid().run)(topology), 222.0)
}
