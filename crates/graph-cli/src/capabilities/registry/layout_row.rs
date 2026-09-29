//! The layout rows: which oracle record and status each registered layout carries.

use super::Capability;
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

/// The six igraph 2D layouts, held to `harness/oracle-igraph.py`'s stress ratio.
const IGRAPH_LAYOUTS: [&str; 6] = [
    "layout.force.fruchterman_reingold",
    "layout.force.kamada_kawai",
    "layout.force.drl",
    "layout.force.lgl",
    "layout.force.davidson_harel",
    "layout.force.graphopt",
];

/// Layouts held to a force-layout oracle rather than to a byte-exact one, keyed by the
/// record their differential writes.
///
/// Force layouts are the project's first layouts that **cannot** be byte-compared
/// against their oracle, and the reason is algorithmic rather than a shortfall: a
/// force simulation amplifies a 1-ULP difference into a different picture, so
/// "different, but no worse" is the strongest true claim available and identity is
/// not. Each therefore gets its own record, holding its own metric:
///
/// - `layout.force.barnes_hut` is held to **d3-force@3.0.0** (the frozen force set at
///   d3's own parameters) by the stress metric: Pearson hop/euclid correlation over 32
///   max-min pivots, margin **-0.05** against d3 on the same topology, the same golden
///   spiral seed positions and the same 112 ticks (`stress`; spec decision 6).
/// - `layout.forceatlas2` is held to **networkx 3.6 `forceatlas2_layout`** in the
///   `ge-python-oracle` image (`oracle-fa2`): a port of the whole function at its
///   defaults, compared with the pinned library rather than restated by hand.
///
/// Both rows are `implemented`, not `gated`: `Status::Gated` is refused by
/// `problems()` unless *both* a 4-way hash verdict and the row's own oracle verdict
/// are backed by a recorded run on this tree, and neither force differential has been
/// run to 1000 seeds here. Claiming `gated` for a force layout with only a hash behind
/// it would be exactly the silent weakening of the project's central guarantee the
/// phase prompt forbids.
fn force_record(id: &str) -> Option<(&'static str, Status)> {
    match id {
        "layout.force.barnes_hut" => Some(("stress", Status::Implemented)),
        "layout.forceatlas2" => Some(("oracle-fa2", Status::Implemented)),
        id if IGRAPH_LAYOUTS.contains(&id) => Some(("oracle-igraph", Status::Implemented)),
        _ => None,
    }
}

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
