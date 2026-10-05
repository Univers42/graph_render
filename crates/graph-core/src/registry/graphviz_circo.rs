//! Ledger metadata for `layout.circular.circo`, Graphviz's `circo`.
//!
//! It lives in its own child module rather than beside `layout.twopi` in `registry/radial.rs`
//! so that two parallel engine jobs touch `registry.rs` with a one-line `mod` and a one-line
//! `LAYOUTS` append each, and so that a third Graphviz engine can be added without this file
//! growing. `RADIAL_CEILING` stays twopi's; a circular layout's cost is not a radial layout's,
//! so this row declares its own.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.circular.circo` was run at.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout
/// layout.circular.circo --n 64,128,256,512,1000,1024,2000,3520 --past-ceiling --repeat 3`:
/// 1.74 ms at 64, 9.55 ms at 128, 149.84 ms at 256, 893.54 ms at 512, 5 742.94 ms at 1 000,
/// 5 557.46 ms at 1 024, 33 981.95 ms at 2 000 and 206 130.58 ms at 3 520 nodes. The full table
/// is in `docs/measurements/p13-gv1-circo.md`.
///
/// **1 000 is this row's real answer and the ceiling is not the largest size `bench` accepts.**
/// A cubic in the largest block makes the two very different questions: the gate's own
/// 2..601-node models all run, and past that the per-node cost climbs by roughly 5x per
/// doubling (149 ms → 894 ms → 5 557 ms → 33 982 ms) rather than staying flat, so 10 000
/// nodes was not run and 1 000 000 was never a claim. The ceiling is the largest size measured
/// with room to spare, 1 000, and the degradation string below says what a caller past it gets.
/// The escape hatch is Graphviz's own: `-Goneblock` skips the crossing reduction entirely,
/// which is what makes the cubic avoidable at the cost of a worse drawing.
pub const GRAPHVIZ_CIRCO_CEILING: u64 = 1_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity \
once an id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation. \
Nothing degrades *within* the ceiling in kind: there is no iteration budget, no cut-off and no \
force model, so a larger graph costs more time and returns the same kind of answer. But the \
cost is not the O(n + m) headline either — the crossing reduction is O(k^3) in the largest \
block's size k, and the measured per-node cost climbs about 5x per doubling past 256 nodes \
(149 ms at 256, 894 ms at 512, 5 557 ms at 1 024, 33 982 ms at 2 000), so a graph whose blocks \
are large rather than numerous costs far more per node than a graph of the same size with many \
small blocks. That cubic is paid, not skipped: the reduction fires on a block whenever the \
reference's own crossing count is non-zero at the long path's order, which is most blocks of \
four nodes or more (docs/measurements/p13-gv1-circo.md §8). Past the ceiling there is no \
truncation and no fallback: the one escape from the cubic is Graphviz's own -Goneblock, which \
skips the crossing reduction and returns a worse drawing, and this port exposes no such knob";

pub(super) const CIRCO: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 circo -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle image \
(pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, compared \
by harness/oracle-graphviz.py in its differential mode: the largest absolute node-coordinate \
difference in points after both arms are rescaled to the same bounding box, measured at \
6.460e+04 points over the sweep (worst: seed 553, n=555) against a ceiling of 1e+05, the next \
power of ten above it. -Gstart is INERT for this engine — measured, twice over a strided \
20-seed subset spanning n=2..552, the output is byte-identical at start=1, 7 and 99 — so the \
comparison is against Graphviz's own answer and not against seed drift. The gap is a circle \
ORDER, not a radius: the block decomposition, the radius N*(min_dist+largest_node)/2*PI and the \
14 analytically determined closed cases (one node, two nodes, a 3-path, a 5-path, a triangle, \
a 4-cycle, a 5-cycle, a 6-cycle, a chorded 5-cycle, K4, K5, a 5-star and two glued blocks) all \
agree, the last group byte for byte at the plain format's own printed precision — ours node by \
node in crates/graph-core/src/layout/graphviz/circo/tests.rs, Graphviz's in the harness — \
because a tolerance is weaker than the truth those cases carry. What differed was which node \
takes which slot on a block's circle, and the cause was the crossing count reduce_edge_crossings \
feeds on: the reference's open-edge set never loses an entry (its remove_edge looks up the other \
Agedge_t of an undirected edge than the one it opened with), so it counts edges the port's \
counter had already retired and moves the circle order where the port's did not; see the \
Ponytail (tie order in the skeleton) marker below and docs/measurements/p13-gv1-circo.md §8",
    complexity: "O(n + m) to find the blocks and O(k^3) to order each one, where k is the \
largest block's node count: per block a lowlink pass, then the skeleton pass (at most k - 3 \
rounds over the node's own edges), a spanning tree, the longest-path walk, a residual pass, and \
up to ten crossing-reduction rounds that each move a node twice per incident edge and recount \
every crossing of the block",
    scale_ceiling: GRAPHVIZ_CIRCO_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (tie order in the skeleton): the degree list remove_pair_edges sorts is \
ordered by a STABLE sort here and by glibc 2.41's qsort there, and the two agree, so a tie is not \
a source of disagreement — measured in the oracle image, 20 of 20 trials at each of n = 5, 20, \
141, 552, 1000, 5000, 10000 and 100000 with keys drawn from four values \
(docs/measurements/p13-gv1-circo.md §8). The 984-of-1000 figure this field used to carry was \
measured against a crossing count that retired each edge when it closed; the reference's own \
remove_edge never retires anything, and that, not this sort, is what moved the circle orders. A \
second and separate cause sat in the same pass and is now fixed: the fan branch of the degree \
top-up counted the fanned neighbour's DEGREE twice, which reorders the very list this marker \
clears (docs/measurements/p13-gv1-circo.md §9). \
Ponytail (disconnected input): Graphviz lays out each connected component \
and then packs them apart with packSubgraphs; this port lays each component out around the \
origin and leaves them overlapping. Failing input: any graph with two components. Direction: \
overlap, the cosmetic one — every node still lands at a finite point on its own block's circle, \
and no edge is mis-drawn. Escape hatch: connect the graph before laying it out; the \
differential's fixtures are connected by construction, so the two arms are only ever compared \
where they agree. Ponytail (oracle resolution): -Tplain prints five significant digits, so the \
comparison cannot be tighter than the printed resolution whatever the layout's f64 arithmetic \
is; the measured gap and the ceiling are in docs/measurements/p13-gv1-circo.md and the ceiling \
is the next power of ten above it. Escape hatch: the 14 closed cases, compared byte for byte \
at the same printed precision, which is where the exactness lives. Ponytail (scale_ceiling): \
measured on the gate's own random graphs up to 3 520 nodes, not a bound on the largest block, \
and not the largest size bench accepts — see GRAPHVIZ_CIRCO_CEILING",
};
