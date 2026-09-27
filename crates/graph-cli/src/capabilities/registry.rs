//! The registered capabilities. Phase 1: the topology layer, eight rows, each naming the
//! oracle functions whose differential backs it.

use super::{Capability, Status};

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
the 4-way hash. Ponytail (orientation): child_of edges enter the hierarchy CSR source-first, i.e. \
the child as parent (Topology::hierarchy); Phase 3 decides before reading it",
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

/// Every registered capability.
pub fn registry() -> Vec<Capability> {
    TOPOLOGY
        .iter()
        .map(|&(id, functions, complexity, ponytail)| Capability {
            id,
            tier: 1,
            stage: "topology",
            geometry: None,
            status: Status::Gated,
            oracle: ORACLE,
            functions,
            hash_stage: "topology",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling: TOPOLOGY_CEILING,
            degradation: DEGRADES,
            ponytail,
            complexity,
        })
        .collect()
}
