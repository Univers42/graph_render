//! Style adapters: the four style rows. One function each, so the row and the style it
//! draws cannot be confused: a table built from one `style_run` call site would let a
//! row's `Style` argument be the only thing separating two capabilities.

use graph_core::Geometry;
use graph_core::StageError;
use graph_core::Topology;
use graph_core::post::Bundled;
pub use graph_core::post::styles::Style;
use graph_core::post::styles::{StyleParams, style_edges};

/// The four style rows. One function each, so the row and the style it draws cannot be
/// confused: a table built from one `style_run` call site would let a row's `Style`
/// argument be the only thing separating two capabilities.
pub fn straight(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Straight)
}

pub fn orthogonal(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Orthogonal)
}

pub fn quadratic(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Quadratic)
}

pub fn bezier(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Bezier)
}

/// One style at its own pinned defaults (`StyleParams::for_style`).
///
/// A style is exact: every edge gets a row, nothing is dropped and nothing is estimated,
/// so `unbundled` is `0` and `pairs` is `0` for the same reason routing's is.
fn style_run(
    topology: &Topology,
    geometry: &Geometry,
    style: Style,
) -> Result<Bundled, StageError> {
    let edges = style_edges(topology, &geometry.nodes, &StyleParams::for_style(style))?;
    Ok(Bundled {
        geometry: Geometry {
            nodes: geometry.nodes.clone(),
            edges,
            notes: geometry.notes.clone(),
        },
        pairs: 0,
        unbundled: 0,
    })
}