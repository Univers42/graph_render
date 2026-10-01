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
const SCIPY_ORACLE_LAYOUTS: [&str; 2] = ["layout.spectral", "layout.mds.pivot"];

/// A layout's row. Tidy tree and treemap are gated on `oracle-layouts` (the d3-hierarchy
/// differential); grid, circular and packing are gated on `roundtrip`'s hand oracle,
/// which records each under its own id. Its hash stage is its id either way.
pub(super) fn layout(layout: &'static core::Capability) -> Capability {
    let m = layout.meta;
    let geometry = NODE_KINDS.iter().find(|(kind, _)| *kind == m.nodes);
    let (oracle_record, status) = match force_record(layout.id) {
        Some(found) => found,
        None => (
            if D3_ORACLE_LAYOUTS.contains(&layout.id) {
                "oracle-layouts"
            } else if SCIPY_ORACLE_LAYOUTS.contains(&layout.id) {
                "oracle-spectral"
            } else {
                "roundtrip"
            },
            Status::Gated,
        ),
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
