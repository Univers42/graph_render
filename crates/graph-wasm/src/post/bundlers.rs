//! The bundler adapters and the node-overlap adapter: thin wrappers over graph-core's entry
//! points.

use graph_core::Geometry;
use graph_core::StageError;
use graph_core::Topology;
use graph_core::post::Bundled;

/// The FDEB bundler, as the registry's row calls it: graph-core's own entry point, under
/// this table's one [`PostRun`](graph_core::post::PostRun) signature. A delegation and
/// nothing more — the two bundlers already publish a `PostRun`, so their rows need no
/// adapter, only a name the table can hold.
pub fn fdeb(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    graph_core::post::fdeb::run(topology, geometry)
}

/// The MINGLE bundler, as the registry's row calls it — see [`fdeb`].
pub fn mingle(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    graph_core::post::mingle::run(topology, geometry)
}

/// The node-overlap pass, as the registry's row calls it — see [`fdeb`].
///
/// **The one POST capability that moves nodes**, and so the one whose adapter is worth
/// reading twice: graph-core's `run` is already a `PostRun`, so this is a delegation like
/// the two above, and the node movement it performs is confined to graph-core's own
/// `Geometry::with_nodes`. Nothing in this crate re-orders a node or writes a position, which
/// is what keeps the hashed snapshot identical on the native and wasm32 arms.
pub fn separate(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    graph_core::post::separate::run(topology, geometry)
}
