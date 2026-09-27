//! The registered capabilities. Phase 1: the topology layer, eight rows, each naming the
//! oracle functions whose differential backs it.

use super::{Capability, Status};

/// Node count past which the topology layer stops being usable, and why it is this one.
///
/// Measured (`docs/measurements/p1-topology-memory.md`): `index_model` holds **442 B per
/// node** at 100 000 synthetic nodes and 154 978 edges — 396 B of columns, CSRs and
/// indices, plus 46 B of string arena. wasm32 addresses at most 4 GiB of linear memory,
/// so 4 GiB / 442 B = 9.7 M nodes, rounded down to two figures. The two `u32` limits
/// bind later: the arena's 2^32 − 1 bytes at ~92 M nodes of this shape, the dense index
/// space at 4.29 G. The arena half is data-dependent (§5.1): longer ids lower this.
pub const TOPOLOGY_CEILING: u64 = 9_700_000;

const ORACLE: &str = "src/core/model (TypeScript, this repo)";

const DEGRADES: &str = "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, index_model refuses with CapacityError once the arena would \
pass 2^32-1 bytes — a refusal, never a wrap or a truncation";

fn topology(
    id: &'static str,
    functions: &'static [&'static str],
    complexity: &'static str,
    ponytail: &'static str,
) -> Capability {
    Capability {
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
    }
}

/// Every registered capability.
pub fn registry() -> Vec<Capability> {
    vec![
        topology(
            "topology.index",
            &[
                "indexModel",
                "emptyModel",
                "nodesEqual",
                "buildSyntheticModel",
                "layoutGroups",
            ],
            "O(n + m)",
            "none owed: exact first-wins dedupe; H9 fixed (group is u32, the oracle's & 0xff is not copied)",
        ),
        topology(
            "topology.csr",
            &["indexModel"],
            "O(n + m)",
            "none owed: counting sort, exact",
        ),
        topology(
            "topology.weights",
            &["applyDegreeWeights", "buildSyntheticModel"],
            "O(n + m)",
            "none owed: libm log1p, bit-equal to the oracle",
        ),
        topology(
            "topology.diff",
            &["diffGraph", "isEmptyPatch", "edgesEqual"],
            "O(n + m)",
            "none owed: exact set difference",
        ),
        topology(
            "topology.neighborhood",
            &["neighborhood", "neighborhoodEdges"],
            "O(reachable n + m)",
            "none owed: exact BFS",
        ),
        topology(
            "topology.edgekind",
            &["edgeKindFromType"],
            "O(len)",
            "ordering-dependent substring classifier: 'note_link_hierarchy' is hierarchy because that test runs first; unknown types silently become relation",
        ),
        topology(
            "topology.legend_counts",
            &["deriveLegend"],
            "O(n + m + d log d + t log t)",
            "none owed: counts only, colour stays in TypeScript (H7)",
        ),
        topology(
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
            "parseNodeId: ':' in source or databaseId shifts the parse to a wrong result, not None (H5); hashString: i32::MIN pinned; makeEdgeId orders by bytes, not localeCompare (H1)",
        ),
    ]
}
