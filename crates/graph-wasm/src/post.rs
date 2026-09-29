//! The POST stage over the ABI (`docs/contract/wasm-abi.md` "POST"): which edge-geometry
//! capability `gm_post_run`'s index names, and the adapters that fit graph-core's own
//! entry points into the one `PostRun` signature.
//!
//! **The table is a table, not a slice of `graph_core::post::POSTS`.** graph-core's own
//! registry holds the two bundlers; routing (`post/routed.rs`) and the four styles
//! (`post/styles.rs`) have their own entry points that take their parameters
//! explicitly, so a row here is a *thin adapter* over one of them rather than a change
//! to a signature graph-core already publishes. Nothing in graph-core is edited for this,
//! and a capability registered there later is one row away.
//!
//! **Every row runs at its own pinned defaults** — `GridParams::default()` for the route,
//! `StyleParams::for_style` for a style — because a hashed result is pinned to a default
//! and a caller reads the id, not a parameter block (the same rule `gm_run` follows, C2).
//!
//! Determinism: the table is a `static` in one fixed order, and every row delegates to
//! graph-core's own deterministic pass. `bundled` is read back only through
//! `graph_core::layout::snapshot`, so nothing here re-orders a node or an edge.

use graph_contract::geometry::EdgeGeometry;
use graph_core::Geometry;
use graph_core::StageError;
use graph_core::Topology;
use graph_core::post::grid_index::GridParams;
use graph_core::post::routed;
use graph_core::post::styles::{Style, StyleParams, style_edges};
use graph_core::post::{Bundled, PostRun};

#[cfg(test)]
mod tests;

/// The routing capability's id, the one this crate names. graph-core's `routed` module
/// does not register itself (it has no `PostRun`-shaped entry point of its own), so the
/// id lives with the adapter that gives it one.
pub const ROUTE_ID: &str = "post.route.grid";

/// One POST capability as the ABI names it: an id a caller discovers through
/// `gm_post_count`/`gm_post_id`, and the run `gm_post_run` calls for that index.
pub struct Entry {
    /// Its capability id, e.g. `post.route.grid`.
    pub id: &'static str,
    /// It at its default parameters — the run a hashed snapshot is pinned to.
    pub run: PostRun,
}

/// Every POST capability the ABI exposes, in registration order: the two bundlers
/// graph-core registers, then routing, then the four styles. Append-only — a row added
/// here is discoverable with no ABI change (the property C1 states for layouts).
pub static CAPABILITIES: [Entry; 7] = [
    Entry {
        id: graph_core::post::fdeb::ID,
        run: graph_core::post::fdeb::run,
    },
    Entry {
        id: graph_core::post::mingle::ID,
        run: graph_core::post::mingle::run,
    },
    Entry {
        id: ROUTE_ID,
        run: route_grid,
    },
    Entry {
        id: Style::Straight.id(),
        run: straight,
    },
    Entry {
        id: Style::Orthogonal.id(),
        run: orthogonal,
    },
    Entry {
        id: Style::Quadratic.id(),
        run: quadratic,
    },
    Entry {
        id: Style::Bezier.id(),
        run: bezier,
    },
];

/// How many POST capabilities the ABI exposes; `gm_post_count`'s body.
pub fn count() -> u32 {
    u32::try_from(CAPABILITIES.len()).unwrap_or(0)
}

/// The id at index `i`, or `None` past the end — `gm_post_id`'s refusal, and the same
/// shape `gm_layout_id` answers out of range with.
pub fn id_at(i: u32) -> Option<&'static str> {
    CAPABILITIES.get(i as usize).map(|entry| entry.id)
}

/// The capability at index `i`, run over `geometry` on `topology`; `None` past the end.
/// The one body `gm_post_run` delegates to, so its two refusals — a bad index and the
/// capability's own error — are both pinned natively.
pub fn run(
    i: u32,
    topology: &Topology,
    geometry: &Geometry,
) -> Option<Result<Bundled, StageError>> {
    CAPABILITIES
        .get(i as usize)
        .map(|entry| (entry.run)(topology, geometry))
}

/// Obstacle-avoiding routing at [`GridParams::default`], as a `PostRun`.
///
/// The straight-segment fallback is reported where a caller can act on it:
/// `Bundled::unbundled` is the route count, which is the same field the bundlers use
/// for "this edge ended up drawn on its own" — one meaning, one field. `pairs` is `0`
/// honestly: routing attracts nothing into a bundle, so there is no pair count to
/// report and inventing one would be a number nobody can act on.
fn route_grid(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    let routed = routed::route(&geometry.nodes, topology.edges(), &GridParams::default())?;
    let unbundled = routed.fallbacks;
    Ok(bundled(
        geometry,
        EdgeGeometry::Polyline(routed.paths()),
        unbundled,
    ))
}

/// The four style rows. One function each, so the row and the style it draws cannot be
/// confused: a table built from one `style_run` call site would let a row's `Style`
/// argument be the only thing separating two capabilities.
fn straight(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Straight)
}

fn orthogonal(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Orthogonal)
}

fn quadratic(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    style_run(topology, geometry, Style::Quadratic)
}

fn bezier(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
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
    Ok(bundled(geometry, edges, 0))
}

/// The result every adapter builds: the layout's nodes and notes **unchanged** (a post
/// pass may not move a node or drop what the layout recorded) and the pass's own edges.
fn bundled(geometry: &Geometry, edges: EdgeGeometry, unbundled: u32) -> Bundled {
    Bundled {
        geometry: Geometry {
            nodes: geometry.nodes.clone(),
            edges,
            notes: geometry.notes.clone(),
        },
        pairs: 0,
        unbundled,
    }
}
