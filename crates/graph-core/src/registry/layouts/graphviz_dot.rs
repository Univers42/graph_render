//! Ledger metadata for Graphviz's layered engine, `layout.dag.dot`.
//!
//! Split out of `registry/layouts.rs` for the house's 300-line cap, and kept as a **child** of
//! `layouts.rs` rather than a sibling of it because this job's allowed paths stop at
//! `registry/**`: declaring `mod graphviz_dot;` from `registry.rs` would be outside them, while
//! a child module of `layouts.rs` lives in `registry/layouts/` and is declared from the file the
//! array is in. Same module, same visibility, one directory deeper.
//!
//! The row is `Status::Implemented` and not `gated`, by the argument
//! `docs/measurements/p13-gv2-dot.md`'s "Ceiling" section makes and `layout.packing.osage`
//! already sets: the layout draws, and a layout that reproduces the oracle only where every node
//! box is the default is not a gated layout. `scale_ceiling` is **not** invented — see below.

use crate::registry::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.dag.dot` was run at.
///
/// **Not set.** The job's rule is that the ceiling is the next power of ten above the worst gap
/// measured over the fixture seeds, and no gap has been measured against this layout, because
/// the graph-cli differential that would measure it is the next job. Writing a number here
/// would be inventing it. What the port's own complexity note does bound is the pass's shape:
/// `r + 2m + 2s` after `k` pivots on the auxiliary graph.
pub const DOT_CEILING: u64 = 0;

const DEGRADATION: &str = "past the ceiling there is nothing to degrade, because no ceiling has \
been measured: see DOT_CEILING. What does bound this layout is memory, and it is bounded the \
same way every layout here is -- wasm32 addresses at most 4 GiB, and natively the snapshot \
refuses with SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes, which is \
a refusal and never a wrap or a truncation. Nothing in the pass has a budget, a cut-off or a \
fallback that returns a *different kind* of answer: the rank pass and the mincross pass are \
bounded by their iteration counts, and the x pass by the simplex's Search_size = 30 cut-off, so \
a larger graph costs more time and returns the same kind of drawing";

pub(super) const DOT: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 dot -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle image \
(pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, compared \
by harness/oracle-dot-probe.py: the rank of every node and the order within every rank are \
derived from the printed coordinates, and the position pass is compared against the printed \
coordinates themselves, byte for byte, because -Tplain writes five significant digits in inches \
(lib/common/output.c:129-141) and nothing finer was printed. THE SIX CLOSED CASES AGREE EXACTLY, \
in the plain format's own frame, and are pinned in \
crates/graph-core/src/layout/graphviz/dot/position_tests.rs: one node at (27, 18); two nodes at \
(27, 90) and (27, 18); a 3-path at (27, 162), (27, 90), (27, 18); a 4-cycle at (54, 234), \
(27, 162), (27, 90), (54, 18); a 5-star at (135, 90), (27, 18), (99, 18), (171, 18), \
(243, 18); a 6-branch at (99, 234), (27, 162), (99, 162), (171, 162), (99, 90), (99, 18). \
OVER THE 1000 SEEDS the pass does NOT agree node for node, and the two numbers say why. Of the \
692 seeds whose ranks agree, 408 agree on every rank's order, and of THOSE 408 only 10 print \
every node centre exactly as the oracle prints it. Two causes, both measured and both recorded: \
(1) node_width(text_width(id)) -- the width table the ADR pins, and the only source of node \
widths this port uses -- returns the 0.75 inch DEFAULT BOX for every id of two or three \
characters, while the oracle prints 0.80475 in = 57.942 pt for n10 and 0.97719 in = 70.358 pt \
for n100; so every seed whose ids reach three characters is drawn with boxes three to sixteen \
points too narrow. The relation that does reproduce the oracle's four node-width rows is the one \
text_width.rs's own module doc names, node = 1.37952 * label_box + 0.30669 inches, and it is \
NOT used because the job's contract names node_width(text_width(id)) as the only source and \
text_width.rs is outside that change; settling it is one constant pair in that module. \
(2) The order of the REAL nodes of a rank can agree while the row the x constraints read does \
not, because a chain dummy's slot inside a rank is invisible to the order sweep: on seed 2 the \
oracle draws n1 and n2 110 points apart, which is the row n1, dummy, n2 -- two constraints of 55 \
-- while this port draws them 72 apart, one constraint. Both are pinned as tests in \
position_tests.rs. Before the position pass the engine's own counts were 692 of 1000 on the \
ranks and 408 of those on every rank's order, which is the FACE of the simplex's optimum rather \
than a defect. -Gstart is INERT for this engine (measured: the same fixture hashes identically \
at start = 1, 7 and 99), so none of the residual is seed drift. Full numbers, per-seed findings \
and commands: docs/measurements/p13-gv2-dot.md",
    complexity: "O(r + 2m + 2s) after k pivots on the auxiliary graph, where r is the node count \
and m the input edge count and s the chain-dummy count -- that is, linear in the size of the \
auxiliary graph plus the pivot loop, which is not bounded in closed form and is what the \
simplex's Search_size = 30 cut-off exists for. The rank and mincross passes before it are the \
same shape and are stated where they are ported. The plain entry point adds one read of the \
input edges and nothing else; the node widths are one table lookup per node",
    scale_ceiling: DOT_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (same-rank edge precedence): the reference's transverse pass refuses to \
swap two nodes joined by an edge whose ends share a rank, through a per-rank adjacency matrix \
built by a depth-first search over those edges; this port answers 'swap allowed' for every pair, \
and dot/position.rs's make_LR_constraints drops the flat-edge constraints for the same reason. \
Failing input: a graph holding an edge whose two ends land on the same rank. Direction: an order \
this port is free to contradict and the reference is not, and with it a different set of x \
positions. Escape hatch: none needed and none possible -- measured over all 1000 fixture seeds, \
NOT ONE has a same-rank edge (0 of 1000, 0 of 400280 edges), so nothing measured here can see it, \
and a graph that does is the failing input. Ponytail (ports): the reference offsets each of an \
edge pair's two lengths by its port's own x, so a ported edge is asked to leave room for where \
it attaches; this port has no ports and no attribute channel to read them from, so both offsets \
are zero and a ported edge is drawn between its nodes' centres. Failing input: any DOT graph \
with a tailport or a headport. Direction: an edge enters and leaves a box instead of a port. \
Escape hatch: none; Fast records no port offset on an edge. Ponytail (clusters): the whole \
cluster path is absent -- pos_clusters, contain_nodes, compress_graph, the cluster half of the \
rank gap in set_ycoords, and the graph-level height below the lowest rank that dot_compute_bb \
subtracts. Failing input: a DOT graph with subgraph cluster_*. Direction: cluster boxes become \
plain nodes in one flat array, and no row is spaced for a cluster label. Escape hatch: none, the \
motor's Topology is a flat node set with no cluster concept. Ponytail (splines): dot_splines is \
not ported; the motor emits polylines through the chain dummies, which is the same shape Graph \
already carries, so the fourth pass has nothing to add here. Ponytail (connectGraph): the \
reference's retry arm, which joins a disconnected graph with zero-length edges and runs the x \
simplex again, is not ported, so a disconnected Topology returns a refusal and no drawing at \
all. Failing input: a disconnected Topology. Direction: no drawing, not a different one. Escape \
hatch: run the pass once per component -- the pieces are independent, which is what the rank pass \
already does per component. Measured out of reach: every one of the 1000 seeded graphs is \
connected. Ponytail (attributes): rankdir, nodesep, ranksep, ratio, nslimit, searchsize and the \
node-attribute channel are all read by the reference and none by this port, so the layout always \
draws the reference's default drawing. Failing input: a DOT graph carrying any of them. \
Direction: the default answer, which is the answer the ledger row and the oracle ask for. Escape \
hatch: a Params on the stage, which is a contract change and not this job's. Ponytail (the node \
width, the one that used to cost the agreement): the width comes from a MEASURED TABLE rather \
than a font engine, and the table's node_width is the formula of record, max(0.75 in, text + 2 * \
0.11 in), which returns the default box for every label of up to three characters. Failing input: \
any graph whose ids are three characters or longer -- that is every fixture seed from 9 on. \
Direction: boxes too narrow by 3.942 points at three characters and by 16.36 at four, so the x \
simplex is asked to satisfy constraints that are that much too short. Escape hatch: one constant \
pair in crates/graph-core/src/layout/graphviz/text_width.rs, which is the single copy the ADR \
names and which this job's allowed paths exclude. Ponytail (scale_ceiling): not measured -- see \
DOT_CEILING",
};