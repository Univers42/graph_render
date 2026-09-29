//! The registered capabilities. Phase 1: the topology layer, eight rows, each naming the
//! oracle functions whose differential backs it. Phase 2: every layout of graph-core's
//! registry, one row each, its metadata taken from there as declared.

use super::post;
use super::{Capability, Status};
use graph_contract::canonical_json::NODE_KINDS;
use graph_core::registry::{self as core, LAYOUTS};

/// Node count past which the topology layer stops being usable, and why it is this one.
///
/// Estimated, not measured on the target (`docs/measurements/p1-topology-memory.md`,
/// reproduced by `crates/graph-core/tests/memory.rs`): natively, `index_model` holds
/// **442 B per node** at 100 000 synthetic nodes and 154 978 edges — 396 B of columns,
/// CSRs and indices, plus 46 B of string arena. wasm32 addresses at most 4 GiB of linear
/// memory, so 4 GiB / 442 B = 9.7 M nodes, rounded down to two figures. The two `u32`
/// limits bind later: the arena's 2^32 − 1 bytes at ~92 M nodes of this shape, the
/// dense index space at 4.29 G. The index row's Ponytail says what the estimate misses.
pub const TOPOLOGY_CEILING: u64 = 9_700_000;

const ORACLE: &str = "src/core/model (TypeScript, this repo)";

const DEGRADES: &str = "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, index_model refuses with CapacityError once the arena would \
pass 2^32-1 bytes — a refusal, never a wrap or a truncation";

/// `(id, oracle functions, complexity, ponytail)` for every topology row.
type Row = (
    &'static str,
    &'static [&'static str],
    &'static str,
    &'static str,
);

const TOPOLOGY: [Row; 8] = [
    (
        "topology.index",
        &[
            "indexModel",
            "emptyModel",
            "nodesEqual",
            "buildSyntheticModel",
            "layoutGroups",
        ],
        "O(n + m)",
        "dedupe exact (first wins); H9 fixed (group is u32, the oracle's & 0xff is not copied). \
Ponytail (scale_ceiling): an estimate — measured natively on 64-bit with synthetic ids and \
projected onto wasm32's 4 GiB; wasm32's 4-byte pointers lower the real cost, longer ids raise it, \
and the caller's input records are not counted. Re-measure with crates/graph-core/tests/memory.rs",
    ),
    (
        "topology.csr",
        &["indexModel"],
        "O(n + m)",
        "builder exact (counting sort), no marker owed. Evidence gap: the oracle compares only \
indexModel's merged adjacency, so the out/in split and the hierarchy CSR rest on unit tests and \
the 4-way hash",
    ),
    (
        "topology.weights",
        &["applyDegreeWeights", "buildSyntheticModel"],
        "O(n + m)",
        "none owed: libm log1p, bit-equal to the oracle",
    ),
    (
        "topology.diff",
        &["diffGraph", "isEmptyPatch", "edgesEqual"],
        "O(n + m)",
        "none owed: exact set difference",
    ),
    (
        "topology.neighborhood",
        &["neighborhood", "neighborhoodEdges"],
        "O(reachable n + m)",
        "none owed: exact BFS",
    ),
    (
        "topology.edgekind",
        &["edgeKindFromType"],
        "O(len)",
        "ordering-dependent substring classifier: 'note_link_hierarchy' is hierarchy because that \
test runs first; unknown types silently become relation",
    ),
    (
        "topology.legend_counts",
        &["deriveLegend"],
        "O(n + m + d log d + t log t)",
        "none owed: counts only, colour stays in TypeScript (H7)",
    ),
    (
        "topology.ids",
        &[
            "makeRecordNodeId",
            "makeNoteNodeId",
            "makeTagNodeId",
            "makeEdgeId",
            "parseNodeId",
            "hashString",
        ],
        "O(len)",
        "parseNodeId: ':' in source or databaseId shifts the parse to a wrong result, not None \
(H5); hashString: i32::MIN pinned; makeEdgeId orders by bytes, not localeCompare (H1)",
    ),
];

/// Every registered capability: the topology rows, then the layouts, then Phase 8's POST
/// rows. Phase 7's `analysis.*` rows are not here: that branch is not merged into this
/// one, and a row with no implementation behind it would be a lie the ledger cannot check.
pub fn registry() -> Vec<Capability> {
    let topology = TOPOLOGY
        .iter()
        .map(|&(id, functions, complexity, ponytail)| Capability {
            id,
            tier: 1,
            stage: "topology",
            geometry: None,
            status: Status::Gated,
            oracle: ORACLE,
            oracle_record: "oracle-diff",
            functions,
            hash_stage: "topology",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling: TOPOLOGY_CEILING,
            degradation: DEGRADES,
            ponytail,
            complexity,
        });
    topology
        .chain(LAYOUTS.iter().map(layout))
        .chain(post::rows())
        .collect()
}

/// Layouts held to `harness/oracle-layouts.mjs`'s d3-hierarchy differential instead of a
/// hand oracle: tidy tree and treemap both restate an exact d3-hierarchy call sequence,
/// so the honest oracle is the library itself, byte-compared after `Math.fround`. Circular
/// and packing have no third-party equivalent to differential-test against (radial
/// placement and circle packing are hand conventions, not d3 calls this phase pins), so
/// they stand on the hand oracle `roundtrip` already checks per seed
/// (`snapshot_cmd::hand_oracles`), same as grid.
const D3_ORACLE_LAYOUTS: [&str; 2] = ["layout.tree.tidy", "layout.treemap.squarified"];

/// A layout's row. Tidy tree and treemap are gated on `oracle-layouts` (the d3-hierarchy
/// differential); grid, circular and packing are gated on `roundtrip`'s hand oracle,
/// which records each under its own id. Its hash stage is its id either way.
fn layout(layout: &'static core::Capability) -> Capability {
    let m = layout.meta;
    let geometry = NODE_KINDS.iter().find(|(kind, _)| *kind == m.nodes);
    let oracle_record = if D3_ORACLE_LAYOUTS.contains(&layout.id) {
        "oracle-layouts"
    } else {
        "roundtrip"
    };
    Capability {
        id: layout.id,
        tier: m.tier,
        stage: m.stage,
        geometry: geometry.map(|(_, name)| *name),
        status: Status::Gated,
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
