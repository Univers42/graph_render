//! The axis swap the layered layouts' `horizontal` parameter applies
//! (`docs/decisions/dag-horizontal.md`): a drawing with x and y exchanged, so the layers run
//! left to right instead of top to bottom.
//!
//! **A pure column swap, no arithmetic.** Node centres, a box's width and height, and every
//! edge point move; offsets, radii, notes and z stay where they are. Nothing is recomputed, so
//! the answer is exact and bit-identical on native and wasm32 (D10) — the property the
//! transposition-equality tests on both layouts rest on.

use super::Geometry;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};

impl Geometry {
    /// This geometry with x and y exchanged: node centres, a box's width and height, and
    /// every edge point. Offsets, radii, notes and z are kept as they are.
    pub(crate) fn transposed(mut self) -> Self {
        self.nodes = transposed_nodes(self.nodes);
        self.edges = transposed_edges(self.edges);
        self
    }
}

fn transposed_nodes(nodes: NodeGeometry) -> NodeGeometry {
    match nodes {
        NodeGeometry::Point { x, y } => NodeGeometry::Point { x: y, y: x },
        NodeGeometry::Circle { x, y, r } => NodeGeometry::Circle { x: y, y: x, r },
        NodeGeometry::Box { x, y, w, h } => NodeGeometry::Box {
            x: y,
            y: x,
            w: h,
            h: w,
        },
    }
}

fn transposed_edges(edges: EdgeGeometry) -> EdgeGeometry {
    match edges {
        EdgeGeometry::Line => EdgeGeometry::Line,
        EdgeGeometry::Polyline(paths) => EdgeGeometry::Polyline(swapped(paths)),
        EdgeGeometry::Curve { degree, paths } => EdgeGeometry::Curve {
            degree,
            paths: swapped(paths),
        },
    }
}

fn swapped(mut paths: Paths) -> Paths {
    for point in paths.pts.as_chunks_mut::<2>().0 {
        point.swap(0, 1);
    }
    paths
}

#[cfg(test)]
mod tests;
