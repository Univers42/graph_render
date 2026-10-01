//! Ledger metadata for the Graphviz packing engines, kept apart from `registry.rs` for
//! the house line cap and out of `registry/radial.rs` because the oracle is neither
//! networkx nor a radial layout: `layout.packing.osage` is Graphviz's own `osage`, reached
//! through the docker-only oracle image (`docs/decisions/graphviz-oracle.md`), and it is
//! closed form over rectangles rather than over a tree.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.packing.osage` was run at, the largest size `graph-cli bench`
/// accepts.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
/// layout.packing.osage --n 220,10000,100000,1000000 --past-ceiling --repeat 3`: 0.00 ms at
/// 220 nodes, 0.03 ms at 10 000, 1.00 ms at 100 000 (154 978 edges) and 8.94 ms at 1 000 000
/// (1 549 929 edges) — a flat ~9 ns per node, and the only reason it is not faster than the
/// clock's resolution is that the layout reads no edges at all. The table is in
/// `docs/measurements/p13-gv1-osage.md`. 1 000 000 is the largest size `bench` accepts and
/// where it was run, not where it was found to stop working, so this is a measured lower
/// bound — nothing about the layout is quadratic.
pub const OSAGE_CEILING: u64 = 1_000_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation. Nothing degrades *within* the ceiling: the layout is closed form with \
no iteration budget, no cut-off and no force model, and it never reads an edge, so a larger \
graph costs more time and returns the same kind of answer";

pub(super) const OSAGE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 osage -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle \
image (pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, \
compared by harness/oracle-graphviz.py --differential: the largest absolute node-coordinate \
difference in points after both arms are rescaled to the same bounding box. THE TWO ARMS DO \
NOT AGREE, and the measurement says so rather than hiding it behind a ceiling: they agree to \
within the oracle's own printed quantum — 3.6e-3 points — on the 18 of 1000 seeds where every \
node box ties (n <= 10), and differ by 122 to 1785 points on the other 982, where they are two \
different drawings. Two named causes, neither an iteration and neither a tolerance: Graphviz \
sizes a node's box from its rendered label (54 points for n0..n9, 57.942 for n10..n99), and \
arrayRects sorts the boxes by width+height with a qsort that is not stable. Worst gap 1.785e3 \
at seed 584, ceiling 1e4 (the next power of ten above it, never widened to pass). -Gstart is \
INERT for this engine (measured: the same fixture hashes identically at start=1, 7, 99), so \
none of the disagreement is seed drift. The six analytically determined small cases (one node, \
two nodes, a 3-path, a 4-cycle, a 5-star and a 6-branch) are additionally compared byte for \
byte at the plain format's own printed precision — ours in \
crates/graph-core/src/layout/graphviz/osage/tests.rs, Graphviz's in the harness — and all six \
match exactly, because six closed cases are all below eleven nodes. Full numbers and commands: \
docs/measurements/p13-gv1-osage.md",
    complexity: "O(n) and no more: the grid size is one sqrt, the gather is one ordered pass \
over the node count, and the edges are never read, so m does not appear at all",
    scale_ceiling: OSAGE_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (node box size — this is the one that costs the agreement): every \
rectangle is taken to be Graphviz's default 0.75 x 0.5 inch nodesize, which holds only while \
every node's label fits inside the minimum, and it stops holding at eleven nodes. Failing \
input: any graph of eleven or more nodes whose ids carry three or more characters, which is \
every graph past the tenth here. Direction: a different drawing, not a worse one — our grid is \
a uniform 58 x 40 where the reference's column widths vary, so rows and columns drift apart by \
up to 1785 points over the sweep. Escape hatch: none inside the motor, and none needed for the \
claim being made: the width is a font metric of Graphviz's own text layout, graph-core has no \
font engine and this stage emits Point geometry with no box at all. What IS held exactly is \
everything below eleven nodes, where all boxes tie — the six closed cases agree byte for byte \
and the sweep gap there is the oracle's printed quantum. Ponytail (qsort tie order): \
arrayRects sorts the rectangles by width+height and glibc's qsort is not stable, so which node \
lands in which cell among tied boxes is the C library's choice; it is declaration order at \
every size measured here and the port assumes it. Direction: a permuted drawing. Ponytail \
(attributes): the reference reads pack, packmode and nodesize and this port reads none, so it \
always packs the array the reference's defaults pack — the default answer, not a different one. \
Failing input: a Graphviz graph carrying packmode=\"node\" or a nodesize. Escape hatch: a Params \
on the stage, a contract change. Ponytail (clusters): the reference packs subclusters \
recursively and this port has no clusters, the motor's Topology being a flat node set. Failing \
input: a DOT graph with subgraph cluster_*. Direction: the clusters become plain nodes in one \
flat array instead of nested boxes — a different drawing, still a valid one. Escape hatch: \
none needed, the motor has no cluster concept. Ponytail (scale_ceiling): a measured lower \
bound — see OSAGE_CEILING",
};
