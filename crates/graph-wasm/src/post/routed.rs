//! Routing adapter: obstacle-avoiding routing over the grid index.

use graph_core::Geometry;
use graph_core::StageError;
use graph_core::Topology;
use graph_core::post::Bundled;
use graph_core::post::grid_index::GridParams;
use graph_core::post::routed;
use graph_contract::geometry::EdgeGeometry;

/// Routing adapter at default parameters.
pub fn route_grid(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    let routed = routed::route(&geometry.nodes, topology.edges(), &GridParams::default())?;
    let unbundled = routed.fallbacks;
    Ok(Bundled {
        geometry: Geometry {
            nodes: geometry.nodes.clone(),
            edges: EdgeGeometry::Polyline(routed.paths()),
            notes: geometry.notes.clone(),
        },
        pairs: 0,
        unbundled,
    })
}