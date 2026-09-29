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
/// Phase 4 adds. `transport.wasm.columnar` is `Gated` on the hash gate's own two
/// verdicts — its `transport.wasm.columnar` stage 4-way equal, and the C20 tally
/// `hashgate.json` records beside it. `sdk.js` is `Implemented`: its gate is
/// `harness/sdk-smoke.mjs`, a smoke script over one fixture rather than a recorded seed
/// sweep, so no record backs it yet and a `gated` claim would be one `--check` has to
/// refuse (`docs/contract/wasm-abi.md` "Ledger").
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

/// The wasm ABI's columnar handle/build/run/column surface. Gated on the hash gate's own
/// evidence: `graph-cli hashgate` hashes the real ABI as a stage of its own
/// (`hashgate/stages.rs`), so a divergence names the transport, and it records the
/// per-seed count of the seeds where that real ABI reached the retained shim's bytes —
/// the C20 acceptance criterion, read back by `verdict::oracle_diff` rather than asserted
/// in prose. Split one row per function (house limit; mirrors `layout`'s one-row-per-call
/// shape below) rather than building both in one.
fn transport() -> [Capability; 2] {
    [transport_wasm_columnar(), transport_sdk_js()]
}

fn transport_wasm_columnar() -> Capability {
    Capability {
        id: "transport.wasm.columnar",
        tier: 1,
        stage: "transport",
        geometry: None,
        status: Status::Gated,
        oracle: "the retained hash-gate shim, through the same module: hashgate's \
transport.wasm.columnar stage (gm_seed_ingest -> gm_alloc -> gm_build -> gm_run -> \
gm_snapshot_bytes) against gm_layout_grid, per seed, natively and on wasm32",
        oracle_record: "wasm-transport",
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
    }
}

fn transport_sdk_js() -> Capability {
    Capability {
        id: "sdk.js",
        tier: 1,
        stage: "sdk",
        geometry: None,
        status: Status::Implemented,
        oracle: "harness/sdk-smoke.mjs: a third party importing only crates/graph-sdk-js's \
published entry point, never the raw wasm exports. It runs every layout Motor#layouts \
reports and asserts the contract's column table per layout, so a newly registered layout is \
covered with no edit to it. Not recorded as evidence, so the row's two verdict columns read \
`not backed` until a sweep writes a record a `gated` claim may stand on",
        oracle_record: "sdk-smoke",
        functions: &["createMotor", "build", "layouts", "layout", "release"],
        hash_stage: "sdk.js",
        oracle_diff: String::new(),
        hash_4way: String::new(),
        scale_ceiling: TRANSPORT_CEILING,
        degradation: SDK_DEGRADES,
        ponytail: "Ponytail (scale_ceiling): not independently measured — see \
transport.wasm.columnar, which this row's ceiling is taken from. Escape hatch: none this phase",
        complexity: "O(n + m), the module it loads",
    }
}

/// Layouts held to `harness/oracle-layouts.mjs`'s d3-hierarchy differential instead of a
/// hand oracle: tidy tree and treemap both restate an exact d3-hierarchy call sequence,
/// so the honest oracle is the library itself, byte-compared after `Math.fround`. Circular
/// and packing have no third-party equivalent to differential-test against (radial
/// placement and circle packing are hand conventions, not d3 calls this phase pins), so
/// they stand on the hand oracle `roundtrip` already checks per seed
/// (`snapshot_cmd::hand_oracles`), same as grid.
const D3_ORACLE_LAYOUTS: [&str; 2] = ["layout.tree.tidy", "layout.treemap.squarified"];

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
        _ => None,
    }
}
/// Layouts held to `harness/oracle-spectral.py`'s scipy/networkx differential, to a
/// measured ceiling rather than byte equality.
const SCIPY_ORACLE_LAYOUTS: [&str; 2] = ["layout.spectral", "layout.mds.pivot"];

/// A layout's row. Tidy tree and treemap are gated on `oracle-layouts` (the d3-hierarchy
/// differential); grid, circular and packing are gated on `roundtrip`'s hand oracle,
/// which records each under its own id. Its hash stage is its id either way.
fn layout(layout: &'static core::Capability) -> Capability {
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
