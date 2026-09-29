//! The two bundler adapters: thin wrappers over graph-core's entry points.

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