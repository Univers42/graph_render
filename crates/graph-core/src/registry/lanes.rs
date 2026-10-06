//! `layout.dag.lanes`'s registry entry, self-contained like `three_d/random3d.rs`'s, so
//! `layouts.rs` only appends one line (append only: the wasm module maps a layout by index).

use super::{Capability, Metadata, params, run_default};
use crate::layout::lanes::{ID, Lanes};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Vertices past which the lanes layout has not been measured.
///
/// Ponytail (scale_ceiling): one host's measurement, not a property of the layout. The native
/// bench in `docs/measurements/dag-lanes.md` drew 1,000,000 vertices in 199.80 ms at load
/// 2.08, five times inside the 1 s target. The algorithm has no budget and degrades in time
/// only, so the figure is a speed claim about a host.
pub const LANES_CEILING: u64 = 1_000_000;

const LANES: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "hand oracle on roundtrip (graph-cli snapshot_cmd::hand_oracles::lanes): the stated \
convention restated with ordered sets instead of heaps, compared bit for bit per seed on node \
x and y, polyline offsets and pts, and notes, at the gate's own node counts. It restates the \
lane rule as well, so it is a transcription check and NOT the evidence for it: the S < lane(v) \
guard of rule D breaks no invariant, and this oracle agrees with the build whether that guard \
is present or not. The evidence is the unit test \
a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge, \
which runs both arms of the rule in one vertex, alongside \
three_lines_forked_from_one_base_share_the_column_waiting_for_it and the oracle's own \
nothing_sits_on_an_edge, which reads lanes and rows rather than the assignment. The \
cycle-breaking \
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
seeded model's undirected graph draws wide); time stays O((n + m) log n). Rule D holds width \
down where the lines converge: an edge shares the target's smallest reserved lane when that \
lane is lower than its own, so a fan-in drawn from fifty sources costs the columns the \
sources occupy and not one column per source. It is not a minimum: where the target's \
reserved lane is HIGHER than the source's own, the source keeps its own lane and the drawing \
stays as wide as it was. The gate model \
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
the rows. Ponytail (lane choice): rule D shares a lane only when it is LOWER than the source's own, \
so a merge converges into the leftmost waiting column and never bends right into one; the \
choice of which column that is, on a merge, depends on the order other vertices' edges \
arrived in, which is a convention and not a crossing minimiser. Lowest-free-lane remains \
greedy; minimal width is not claimed; a wider drawing is cosmetic. Escape hatch: none needed \
— every edge is still routed, and the width is measured in docs/measurements/dag-lanes.md. Ponytail (scale_ceiling): one host's \
measurement, not a property of the layout — the native bench in docs/measurements/dag-lanes.md \
drew 1,000,000 vertices in 199.80 ms at load 2.08; the algorithm itself has no budget and \
degrades in time only. Escape hatch: re-run the bench and lower the figure to the \
largest measured n under 1 s",
};

/// `LAYOUTS` appends this after `layout.dag.dot`.
pub(in crate::registry) const LANES_LAYOUT: Capability = Capability {
    id: ID,
    run: run_default::<Lanes>,
    params: &params::LANES,
    meta: LANES,
};
