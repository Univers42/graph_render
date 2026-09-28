//! The registered capabilities. Phase 1: the topology layer, eight rows, each naming the
//! oracle functions whose differential backs it. Phase 2: every layout of graph-core's
//! registry, one row each, its metadata taken from there as declared.

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

/// Node count past which the provisional-ingest transport path stops being usable
/// (`docs/measurements/phase04-transport.md`, `crates/graph-wasm/src/memory_measure.rs`):
/// measured natively, the counted span from ingest JSON text already in memory through
/// `ingest::read`, `index_model`, `layout.grid`'s run and the encoded snapshot bytes —
/// exactly `gm_build` + `gm_run` + `gm_snapshot_bytes` — peaks at **2862.3 B per node** at
/// 100 000 synthetic nodes and 154 978 edges. wasm32 addresses at most 4 GiB, so
/// 4 GiB / 2862.3 B ≈ 1.50 M nodes, rounded down to two figures. Heavier than
/// `layout.grid`'s own ceiling: the JSON parse tree and the intermediate `NodeRecord`/
/// `EdgeRecord` vectors this row's own path holds (and the grid pipeline's own
/// measurement does not) cost more than the pipeline itself.
pub const TRANSPORT_CEILING: u64 = 1_500_000;

const TRANSPORT_DEGRADES: &str = "past the ceiling wasm32 cannot allocate and gm_build returns 0 \
with AllocFailed (never a partial handle); gm_alloc's own reservation can fail earlier still, for \
the same reason, on a large ingest buffer alone";

const SDK_DEGRADES: &str = "the SDK holds no per-node memory of its own: its column views are \
zero-copy typed-array aliases over transport.wasm.columnar's buffers, so it degrades exactly when \
the module it loads does — same ceiling, not independently measured in JS this phase";

/// Every registered capability: the topology rows, the layouts, then the transport rows
/// Phase 4 adds. Both new rows are `Implemented`, never `Gated`: this phase records no
/// `hashgate.json`/`oracle-diff.json` entry under either row's `hash_stage`/
/// `oracle_record` name, so `ledger()` reads them back as `not backed`, honestly, rather
/// than a claim `--check` would have to refuse (`docs/contract/wasm-abi.md` "Ledger").
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
        .chain(transport())
        .collect()
}

/// The two Phase 4 transport rows: the wasm ABI's columnar handle/build/run/column
/// surface, and the JS SDK that wraps it.
fn transport() -> [Capability; 2] {
    [
        Capability {
            id: "transport.wasm.columnar",
            tier: 1,
            stage: "transport",
            geometry: None,
            status: Status::Implemented,
            oracle: "harness/wasm-run.mjs hash mode: gm_seed_ingest -> gm_alloc -> gm_build -> \
gm_run -> gm_snapshot_bytes, compared byte for byte against the retained gm_layout_grid shim \
(the same pipeline through the old hash-gate-only exports)",
            oracle_record: "wasm-run-hash",
            functions: &[
                "gm_build",
                "gm_run",
                "gm_column_ptr",
                "gm_column_len",
                "gm_snapshot_bytes",
            ],
            hash_stage: "transport.wasm.columnar",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling: TRANSPORT_CEILING,
            degradation: TRANSPORT_DEGRADES,
            ponytail: "Ponytail (scale_ceiling): measured natively (crates/graph-wasm/src/\
memory_measure.rs) and projected onto wasm32's 4 GiB, not re-measured on the wasm32 target \
itself. Escape hatch: none this phase — Phase 10 owns the real ingest contract and may cost \
differently",
            complexity: "O(n + m) in the ingest JSON's size",
        },
        Capability {
            id: "sdk.js",
            tier: 1,
            stage: "sdk",
            geometry: None,
            status: Status::Implemented,
            oracle: "harness/sdk-smoke.mjs: a third party importing only crates/graph-sdk-js's \
published entry point, never the raw wasm exports",
            oracle_record: "sdk-smoke",
            functions: &["createMotor", "build", "layout", "release"],
            hash_stage: "sdk.js",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling: TRANSPORT_CEILING,
            degradation: SDK_DEGRADES,
            ponytail: "Ponytail (scale_ceiling): not independently measured — see \
transport.wasm.columnar, which this row's ceiling is taken from. Escape hatch: none this phase",
            complexity: "O(n + m), the module it loads",
        },
    ]
}

/// A layout's row. Its hand oracle is checked per seed by `roundtrip`, which records it
/// under the layout's id; its hash stage is its id.
fn layout(layout: &'static core::Capability) -> Capability {
    let m = layout.meta;
    let geometry = NODE_KINDS.iter().find(|(kind, _)| *kind == m.nodes);
    Capability {
        id: layout.id,
        tier: m.tier,
        stage: m.stage,
        geometry: geometry.map(|(_, name)| *name),
        status: Status::Gated,
        oracle: m.oracle,
        oracle_record: "roundtrip",
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
