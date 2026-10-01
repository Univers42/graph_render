//! POST (`prompt.md` §3): the edge-geometry stage. Every capability here takes a finished
//! layout's geometry and returns geometry again, so each one composes with every layout
//! without new code — the reference's separation, where layout returns positions and every
//! edge-path concern lives downstream in `mesh/edge_styles.py` and
//! `engine/scigraphs_engine/bundling/*`.
//!
//! Two bundlers are in the tree: [`fdeb`], force-directed edge bundling in gather form
//! (D10), and [`mingle`], multilevel ink-minimising bundling, greedy and single-threaded.
//! [`ink`] is the measurement that says whether either did anything: the cells a drawing
//! covers, and the length it draws. [`grid_index`] is a uniform grid over the node
//! geometry with node cells marked as obstacles, and [`routed`] routes edges around them. [`styles`]
//! draws parallel edges and self-loops apart.
//!
//! **Scaffolding, additive.** The styles and routing slices add their own `pub mod` line
//! and their own row in [`POSTS`]; the matrix in [`tests`] is written to grow with them.

pub mod fdeb;
pub mod grid_index;
pub mod ink;
pub mod mingle;
pub mod routed;
pub mod styles;
#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry};
use ink::Ink;

/// What the ledger says about a POST capability. Every field is required and none is an
/// `Option`, exactly as [`crate::registry::Metadata`] is for a layout: a capability cannot
/// be registered without declaring its tier, edges, oracle, complexity, `scale_ceiling`,
/// `degradation` and `ponytail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Metadata {
    /// Delivery tier.
    pub tier: u8,
    /// The edge geometry kind it emits.
    pub edges: EdgeGeometryKind,
    /// The reference it is checked against.
    pub oracle: &'static str,
    /// Time complexity, stated and held.
    pub complexity: &'static str,
    /// Edge count past which it stops being usable.
    pub scale_ceiling: u64,
    /// What happens past the ceiling.
    pub degradation: &'static str,
    /// Its Ponytail markers, or the reason none is owed.
    pub ponytail: &'static str,
}

/// One registered POST capability.
#[derive(Debug, Clone, Copy)]
pub struct Capability {
    /// Its capability id, e.g. `post.bundle.fdeb`, which is also its ledger row.
    pub id: &'static str,
    /// It at its default parameters — the run a hashed snapshot is pinned to.
    pub run: PostRun,
    /// Its ledger metadata.
    pub meta: Metadata,
}

/// A post capability's entry point: a layout's geometry in, geometry out. The signature
/// is the composability claim in one line — nothing here is told which layout produced the
/// positions, or which node kind it emitted.
pub type PostRun = fn(&Topology, &Geometry) -> Result<Bundled, StageError>;

/// What a bundler produced: the layout's nodes with its edges replaced by paths, and the
/// two counts that say how much work it did.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundled {
    /// The layout's node geometry and notes, unchanged, and the bundled edge paths.
    pub geometry: Geometry,
    /// FDEB: edge pairs whose compatibility cleared the threshold, the pairs that ever
    /// attract. MINGLE: merges accepted, over every pass of every round.
    pub pairs: u32,
    /// Edges that merged with nothing and are therefore drawn unbundled. The failure the
    /// `post.bundle.fdeb` Ponytail marker names.
    pub unbundled: u32,
}

/// Every registered POST capability, in registration order.
pub static POSTS: [Capability; 2] = [
    Capability {
        id: fdeb::ID,
        run: fdeb::run,
        meta: fdeb::META,
    },
    Capability {
        id: mingle::ID,
        run: mingle::run,
        meta: mingle::META,
    },
];

/// The POST capability registered under `id`.
pub fn find(id: &str) -> Option<&'static Capability> {
    POSTS.iter().find(|cap| cap.id == id)
}

/// Every node's centre, whatever node kind the layout emitted: the three kinds differ in
/// the columns they add, never in the two they share, so a POST pass never needs to know
/// the layout's shape.
///
/// A 3D snapshot's z column is **not** here, and cannot be: `z` lives on the snapshot, not
/// on `NodeGeometry`, and a POST pass is handed `&NodeGeometry`. So a pass reads `x` and `y`,
/// writes edge geometry, and has no way to reach the z column at all — which is what makes
/// z survive every pass unchanged (`docs/decisions/contract-3d-verdict.md` condition 6).
/// POST and SCALE are 2D-only for now; a pass that ever needs z must be given it
/// explicitly rather than finding it by accident.
pub fn centres(nodes: &NodeGeometry) -> (&[f32], &[f32]) {
    match nodes {
        NodeGeometry::Point { x, y } | NodeGeometry::Circle { x, y, .. } => (x, y),
        NodeGeometry::Box { x, y, .. } => (x, y),
    }
}

/// Edge `e`'s interior points as one `x, y, x, y` slice; empty for a `Line`, which stores
/// nothing and takes its endpoints from the node geometry.
pub(crate) fn row(edges: &EdgeGeometry, e: u32) -> &[f32] {
    let (EdgeGeometry::Polyline(paths) | EdgeGeometry::Curve { paths, .. }) = edges else {
        return &[];
    };
    let (from, to) = (
        paths.offsets[e as usize] as usize,
        paths.offsets[e as usize + 1] as usize,
    );
    &paths.pts[2 * from..2 * to]
}

/// The ink `geometry` draws over `topology`'s edges, as a raster's cell count and the
/// total drawn length. Re-exported here so a caller measures before and after with one
/// import.
pub fn measure(topology: &Topology, geometry: &Geometry) -> Ink {
    ink::ink(topology, geometry)
}
