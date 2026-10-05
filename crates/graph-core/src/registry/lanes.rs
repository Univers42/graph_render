//! `layout.dag.lanes`'s registry entry, self-contained like `three_d/random3d.rs`'s, so
//! `layouts.rs` only appends one line (append only: the wasm module maps a layout by index).

use super::{Capability, Metadata, params, run_default};
use crate::layout::lanes::{ID, Lanes};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Vertices past which the lanes layout has not been measured.
///
/// Ponytail (scale_ceiling): a projection, not a measurement. The native bench in
/// `docs/measurements/dag-lanes.md` is the only thing that can lower or raise it, and it had
/// not run when this row shipped. The algorithm itself has no budget and degrades in time
/// only, so the figure is a speed claim about a host, not a property of the layout.
pub const LANES_CEILING: u64 = 1_000_000;

const LANES: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "hand oracle on roundtrip (graph-cli snapshot_cmd::hand_oracles::lanes): the stated \
convention restated with ordered sets instead of heaps, compared bit for bit per seed on node \
x and y, polyline offsets and pts, and notes, at the gate's own node counts; the cycle-breaking \
and version-ordering paths the gate model cannot reach are covered by the unit tests \
a_directed_cycle_is_broken_at_the_lowest_index_and_noted, \
distinct_versions_break_a_cycle_and_the_heap_orders_by_version and \
equal_versions_fall_back_to_index_order; lane width measured in \
docs/measurements/dag-lanes.md",
    complexity: "O((n + m) log n): one counting sort of the directed edges, Kahn with a ready \
heap, one counting sort of the edges by their earlier row, and a min-heap of free lanes; \
O(n + m) memory, no dummy vertices",
    scale_ceiling: LANES_CEILING,
    degradation: "none in routing: every edge has at most two interior points and none is left \
unrouted. Width grows with the number of lines alive at once, up to n on an antichain (the \
seeded model's undirected graph draws wide); time stays O((n + m) log n). The gate model \
exercises the tie-break path only: its arcs are already a topological order and every version \
is 0.0, so no cycle is broken and no two versions are compared — the cycle-breaking and \
version-ordering paths are covered by the unit tests named in `oracle`. Note 5 is carried by \
directed edges only, unlike `layout.dag.sugiyama`'s invariant checker, which notes every \
non-loop edge drawn head to tail: an undirected edge has no head to tail to violate, and the \
geometry helper reverses the point order so the drawing is correct either way",
    ponytail: "Ponytail (row tie-break): ready vertices go largest version first, then lowest \
index; a convention, not a crossing minimiser; equal versions interleave by index, which adds \
lane switches, cosmetic. Escape hatch: none needed — every edge is still routed. Ponytail \
(cycle breaking): a directed cycle is broken at the lowest-index unplaced vertex, so more \
edges than a minimum feedback arc set are drawn head to tail, each with note 5, none lost. \
Escape hatch: read note 5 off the snapshot; its presence names the edges that run against \
the rows. Ponytail (lane choice): lowest-free-lane is greedy; minimal width is not claimed; a \
wider drawing is cosmetic. Escape hatch: none needed — every edge is still routed, and the \
width is measured in docs/measurements/dag-lanes.md. Ponytail (scale_ceiling): a projection, \
not a measurement — the native bench in docs/measurements/dag-lanes.md is the only thing that \
can lower or raise it, and it had not run when this row shipped; the algorithm itself has no \
budget and degrades in time only. Escape hatch: re-run the bench and lower the figure to the \
largest measured n under 1 s",
};

/// `LAYOUTS` appends this after `layout.dag.dot`.
pub(in crate::registry) const LANES_LAYOUT: Capability = Capability {
    id: ID,
    run: run_default::<Lanes>,
    params: &params::LANES,
    meta: LANES,
};
