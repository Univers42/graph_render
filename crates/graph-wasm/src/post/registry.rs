//! The post capability registry: the table of capabilities the ABI exposes, and the
//! functions to query and run them.

use super::bundlers::{fdeb, mingle, separate as separate_pass};
use super::routed::route_grid;
use super::styles::{Style, bezier, orthogonal, quadratic, straight};
use graph_contract::binary::Snapshot;
use graph_core::Geometry;
use graph_core::StageError;
use graph_core::Topology;
use graph_core::post::routed;
use graph_core::post::{Bundled, PostRun};

/// The routing capability's id, the one this crate names — graph-core's own
/// [`routed::ID`](graph_core::post::routed::ID), not a spelling here. graph-core's
/// `routed` module does not register itself (it has no `PostRun`-shaped entry point of its
/// own), but it still owns the id, so the hash gate's stage list and this table name the
/// same string from the same place.
pub const ROUTE_ID: &str = routed::ID;

/// Every POST capability the ABI exposes, in registration order: the two bundlers, then
/// routing, then the four styles, then the node-overlap pass. Append-only — a row added
/// here is discoverable with no ABI change (the property C1 states for layouts), and a row
/// added *at the end* of this table moves no index that already means something.
///
/// **The node mover is last on purpose.** It was first registered between `mingle` and
/// `route`, which read as the tidier order and was wrong: an index that already meant
/// something moved, and nine wasm tests that resolve an id by position went on to run
/// the wrong capability — a routing test asserting straight edges, a fallback count of 0
/// where 3 was the claim. "Append-only" is the whole rule; the order inside it is not a
/// matter of taste.
pub static CAPABILITIES: [Entry; 8] = [
    Entry {
        id: graph_core::post::fdeb::ID,
        run: fdeb,
    },
    Entry {
        id: graph_core::post::mingle::ID,
        run: mingle,
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
    Entry {
        id: graph_core::post::separate::ID,
        run: separate_pass,
    },
];

/// One POST capability as the ABI names it: an id a caller discovers through
/// `gm_post_count`/`gm_post_id`, and the run `gm_post_run` calls for that index.
pub struct Entry {
    /// Its capability id, e.g. `post.route.grid`.
    pub id: &'static str,
    /// It at its default parameters — the run a hashed snapshot is pinned to.
    pub run: PostRun,
}

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

/// The capability at index `i` run over `geometry` on `topology`, as the snapshot it
/// replaces that handle's with; `None` past the end.
///
/// **The one byte path a POST stage is hashed by.** `gm_post_run` is this call plus the
/// handle-table write, and graph-cli's native hashgate arm reads it too — so the two arms
/// compare *this* implementation's bytes rather than two implementations' agreement, and a
/// divergence means wasm32 computed something different from what it would compute natively
/// (D1) rather than that two formatters were written twice and drifted.
pub fn snapshot(
    i: u32,
    topology: &Topology,
    geometry: &Geometry,
) -> Option<Result<Snapshot, StageError>> {
    let ran = run(i, topology, geometry)?;
    Some(ran.and_then(|bundled| graph_core::layout::snapshot(topology, bundled.geometry)))
}
