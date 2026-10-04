//! The layout rows: which oracle record and status each registered layout carries.

use super::Capability;
use super::unproven::force_record;
use crate::capabilities::Status;
use graph_contract::canonical_json::NODE_KINDS;
use graph_core::registry as core;

/// Layouts held to `harness/oracle-layouts.mjs`'s d3-hierarchy differential instead of a
/// hand oracle: tidy tree and treemap both restate an exact d3-hierarchy call sequence,
/// so the honest oracle is the library itself, byte-compared after `Math.fround`. Circular
/// and packing have no third-party equivalent to differential-test against (radial
/// placement and circle packing are hand conventions, not d3 calls this phase pins), so
/// they stand on the hand oracle `roundtrip` already checks per seed
/// (`snapshot_cmd::hand_oracles`), same as grid.
const D3_ORACLE_LAYOUTS: [&str; 2] = [
    graph_core::layout::tidy_tree::ID,
    graph_core::layout::treemap::ID,
];

/// Layouts held to `harness/oracle-spectral.py`'s scipy/networkx differential, to a
/// measured ceiling rather than byte equality.
///
/// **The 3-D arms are here, and they are still routed to `scigraphs-conformance`.**
/// `layout.spectral3d` and `layout.mds.pivot3d` are the same two run functions as their 2-D
/// siblings at `dims = 3`, and `harness/oracle-spectral.py` now pins all four ids, so the
/// pair belongs in this list rather than falling out of it. They do not reach the
/// `oracle-spectral` row, because `force_record` is matched first
/// (`unproven.rs:213`) and names `scigraphs-conformance` for them — which is correct and
/// deliberate: `scripts/scigraphs-conformance.sh` writes that record by byte comparison over
/// the conformance fixtures, a stronger claim than this differential's subspace angle, and a
/// row must not be moved onto a weaker one. Listing them keeps the declaration honest if
/// that ever changes.
const SCIPY_ORACLE_LAYOUTS: [&str; 4] = [
    "layout.spectral",
    "layout.mds.pivot",
    "layout.spectral3d",
    "layout.mds.pivot3d",
];

/// Layouts held to the hand oracle `roundtrip`, which records each under its own id
/// (`snapshot_cmd::hand_oracles`).
///
/// **Named, not defaulted.** These four used to reach `roundtrip` by falling out of the
/// `if/else` chain below, which meant a layout id nobody had thought about inherited the
/// same `gated` claim on the same record. An id in no arm at all is now
/// [`Status::Implemented`] naming a record no gate writes, so registering a layout is a
/// decision somebody makes here rather than a claim it picks up.
const ROUNDTRIP_LAYOUTS: [&str; 4] = [
    "layout.grid",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.dag.sugiyama",
];

/// The record a row of no consequence names when it names no oracle at all. No gate
/// writes a record under this name, so `oracle_diff` reads it as "not backed: no
/// unproven record: run the gate" — which is the truth about an id no arm declares.
pub(super) const UNPROVEN_RECORD: &str = "unproven";

/// Whether an id is named by one of this file's arms, whatever the arm says. The test
/// that fails when a layout is registered in graph-core and no arm here names it — so it
/// exists for tests only, and is compiled only there.
#[cfg(test)]
pub(in crate::capabilities) fn is_declared(id: &str) -> bool {
    force_record(id).is_some()
        || D3_ORACLE_LAYOUTS.contains(&id)
        || SCIPY_ORACLE_LAYOUTS.contains(&id)
        || ROUNDTRIP_LAYOUTS.contains(&id)
}

/// A layout's row. Tidy tree and treemap are gated on `oracle-layouts` (the d3-hierarchy
/// differential); grid, circular and packing are gated on `roundtrip`'s hand oracle,
/// which records each under its own id. Its hash stage is its id either way.
///
/// **Fail-closed**, and this is the shape of the fix: the arms are enumerated above and
/// an id in none of them is `implemented`. It used to be the `else` of the chain, which
/// stamped a newly registered layout `gated` on `roundtrip` with no record of its own.
pub(in crate::capabilities) fn layout(layout: &'static core::Capability) -> Capability {
    let m = layout.meta;
    let geometry = NODE_KINDS.iter().find(|(kind, _)| *kind == m.nodes);
    let (oracle_record, status) = match force_record(layout.id) {
        Some(found) => found,
        None if D3_ORACLE_LAYOUTS.contains(&layout.id) => ("oracle-layouts", Status::Gated),
        None if SCIPY_ORACLE_LAYOUTS.contains(&layout.id) => ("oracle-spectral", Status::Gated),
        None if ROUNDTRIP_LAYOUTS.contains(&layout.id) => ("roundtrip", Status::Gated),
        None => (UNPROVEN_RECORD, Status::Implemented),
    };
    Capability {
        id: layout.id,
        tier: m.tier,
        stage: m.stage,
        geometry: geometry.map(|(_, name)| *name),
        status,
        oracle: m.oracle,
        oracle_record,
        functions: std::slice::from_ref(&layout.id),
        hash_stage: layout.id,
        oracle_diff: String::new(),
        hash_4way: String::new(),
        scale_ceiling: m.scale_ceiling,
        degradation: m.degradation,
        ponytail: m.ponytail,
        complexity: m.complexity,
    }
}
