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
//! **Two registries.** [`POSTS`] holds the capabilities whose entry points take nothing but
//! the geometry: the two bundlers, and [`separate`]. Routing ([`routed`]) and the four styles
//! ([`styles`]) take their parameters explicitly and have no row here: their ledger rows are
//! graph-cli's `capabilities/post.rs`, their ABI rows graph-wasm's `post.rs`.
//! [`grid_index`] is routing's obstacle grid, not a capability. The matrix in `tests`
//! reads [`POSTS`], so it covers the bundlers and [`separate`].
//!
//! **One pass here moves nodes.** [`separate`] pushes node discs apart, which the contract
//! this module used to state flatly did not allow, so the rule is now
//! [`Metadata::moves_nodes`] — declared per capability and asserted per capability in
//! `tests::check_output`, and true for exactly one row. Nothing else about a post pass
//! changed: same signature, same [`Bundled`], same registries, same ordering.
//! `docs/decisions/node-overlap.md` carries the decision and its ruling.

pub mod fdeb;
pub mod grid_index;
pub mod ink;
pub mod mingle;
pub mod routed;
pub mod separate;
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
    ///
    /// **Pass-through for a `moves_nodes` row.** `post::separate` hands back the edges it
    /// was given and moves only node columns, so its row names `Line` as the shape a
    /// composed drawing normally has; there is no `EdgeGeometryKind` for "the same edges",
    /// and inventing one would widen the wire enum for one row. Read `moves_nodes` first:
    /// a row with it set is not claiming what it emits so much as what it leaves alone.
    pub edges: EdgeGeometryKind,
    /// Whether this pass may move a node.
    ///
    /// **This field is why the contract is a field and not an assumption.** POST's rule used
    /// to be that a post pass never moves a node, asserted over the whole registry in
    /// `tests::check_output`. `post::separate` has to break that rule to exist, so the rule
    /// became this: every pass declares what it does, and the matrix asserts the strong
    /// claim — node columns byte-identical, z carried — for every row declaring `false`, and
    /// a weaker stated one for a row declaring `true`. Decided in
    /// `docs/decisions/node-overlap.md` §1 and §3, ruled by the `devil` agent.
    pub moves_nodes: bool,
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

/// What a post pass produced: a geometry, and the two counts that say how much work it did.
///
/// The counts are **per-pass semantics, read from the pass that set them** — that is the
/// established pattern here, not a new one: `pairs` already meant different things to FDEB
/// and MINGLE before anything could move a node. So [`separate`] sets `pairs` to the
/// overlapping node pairs its sweeps resolved and `unbundled` to the pairs still overlapping
/// by more than [`separate::TOLERANCE`] when the iteration cap was reached.
#[derive(Debug, Clone, PartialEq)]
pub struct Bundled {
    /// The layout's node geometry, notes and z column, and the pass's edge paths.
    ///
    /// Every bundler rebuilds through [`Geometry::with_edges`], so a 3D layout keeps its
    /// dimension through the pass or the pass does not run: nothing here drops a column, and
    /// nothing needs to. [`separate::META`] declares [`Metadata::moves_nodes`], and it
    /// rebuilds through [`Geometry::with_nodes`] instead — the only constructor that replaces
    /// node columns, and reachable only from a pass that declared the flag. It refuses a z
    /// column outright rather than moving `x`/`y` under one.
    pub geometry: Geometry,
    /// FDEB: edge pairs whose compatibility cleared the threshold, the pairs that ever
    /// attract. MINGLE: merges accepted, over every pass of every round. SEPARATE: overlapping
    /// node pairs a sweep resolved, summed over the sweeps that ran.
    pub pairs: u32,
    /// Edges that merged with nothing and are therefore drawn unbundled. FDEB: edges no
    /// pair cleared the threshold with, the failure its Ponytail marker names. MINGLE: edges
    /// that never joined a bundle ([`mingle::Bundles::unbundled`]).
    ///
    /// SEPARATE: **node pairs still overlapping by more than [`separate::TOLERANCE`] when
    /// the iteration cap was reached** — the residue the sweeps could not remove, and the
    /// number a caller reads instead of trusting the cap. It is 0 when the sweep converged,
    /// which is the normal case.
    pub unbundled: u32,
}

/// Every registered POST capability, in registration order.
pub static POSTS: [Capability; 3] = [
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
    // Appended, never inserted: graph-wasm's `CAPABILITIES` table and the hash gate's stage
    // list both resolve a capability by index, so a row inserted above these two would move
    // what an index already means. `separate` is also the one row with `moves_nodes: true`.
    Capability {
        id: separate::ID,
        run: separate::run,
        meta: separate::META,
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
