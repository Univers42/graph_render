//! The two layouts of this group that read the graph: [`HIERARCHICAL_3D`], which BFSes it,
//! and [`SPRING_3D`], which iterates. Split out of the parent for the house line cap.

use super::super::Metadata;
use super::{BASIC_3D_CEILING, DEGRADATION};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

pub const HIERARCHICAL_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _hierarchical_layout_3d \
(SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:113-147) at the dispatcher's own \
scale=5.0, ported whole: _component_roots (hierarchical.py:31-52), _multi_source_levels \
(:54-81), _group_by_level (:83-88), per level z = (level / max(1, max_level)) * scale * 2 - scale \
(:140) and radius = scale * 0.5 * sqrt(count / widest) (:142), and _disk_positions (:90-111) with \
rings = max(1, round(sqrt(count/pi))), share = arange(rings)+0.5, take = round(share/share.sum() * \
count) and BOTH fix-up loops, ring radius r = radius*(k+0.5)/rings. The reference's own \
docstring is STALE and the code is authoritative: it claims 'BFS depth sets Y, each level fills a \
disk in XZ' while the code puts the level on z (:140, :144) and the disk in x/y, so this port \
does. Two numpy behaviours are load-bearing and written out rather than approximated: np.round \
is TIES TO EVEN (at count = 10 the reference's own take is [2, 8], where f64::round plus the \
reference's own fix-up loop lands on [3, 7] — a different picture, not a different rounding), and \
np.argmax returns the FIRST maximum (a tie debits the lower ring; Iterator::max would debit \
the last). TWO THINGS SETTLED RATHER THAN ASSUMED. (1) The `widest` denominator on a one-level \
graph: the reference guards z's denominator with max(1, max_level) and uses `widest` bare \
(:140, :142), and `widest` cannot be 0 on this path because num_nodes == 0 returns at :123 and \
every other node gets a level, so the port applies max(1) anyway — a provable no-op that makes \
the division total without a second reasoning chain. (2) The BFS tie-break order, which is a \
D2 fixed-order requirement: nodes are visited in SimpleGraph row order (first-by-ascending-edge- \
index, the insertion order nx.Graph.neighbors yields) and, inside a level, the bucket is in BFS \
DISCOVERY order; a level tie is broken by that discovery order, and a tie between two nodes \
reached at the same depth by the same sweep is broken by ascending edge index. Ties never reach \
a HashMap: every table is indexed by dense index or by level. COMPARED against the SciGraphs arm \
by harness/oracle-hierarchical-3d.py, as coordinates within 1e-6, over the gate's own model. NO \
SEED IS OWED: the function draws no random number. The one stated departure: a digraph is read \
undirected (see ponytail)",
    complexity: "O(n + m) for the roots (two BFS sweeps per component) and the multi-source \
BFS, then O(n) to place",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (directed input): the failing input is a CYCLE in the digraph, or any \
component with no in-degree-0 source — SciGraphs branches on G.is_directed() \
(hierarchical.py:127-128), takes in-degree-0 SOURCES as the roots, and then means SUCCESSORS by \
neighbours, which also changes what a level is. The motor's Topology carries directedness PER \
EDGE and has no whole-graph flag, so there is no such predicate to branch on: this port takes \
the undirected branch, _component_roots, and does not port the in-degree-0 root branch, its \
no-source fallback or successor-only traversal. Direction: COSMETIC AND VISIBLE, never \
wrong-but-plausible spacing — under the undirected branch every node is reachable from its own \
component's root, so nothing piles up; the re-seed loop that would leave unreached nodes at the \
origin is ported (hierarchical.py:75-80) but is unreachable through `run`, and the branch that \
IS reachable draws a rooted tree rather than the digraph's own flow, so a cycle's members land \
at levels by undirected distance instead of by reachability. Escape hatch: repair the digraph \
into a forest UPSTREAM, where the caller already owns the direction — the sibling port says \
the same at registry/hierarchy.rs. Ponytail (dict order): _group_by_level builds \
{level: [node, ...]} by iterating a dict (hierarchical.py:83-88), so the order is a \
property of CPython's insertion ordering rather than something the reference states; the port \
reproduces it by iterating ASCENDING LEVEL and BFS DISCOVERY order inside a level, which is the \
same sequence — a level key is first created before any larger one — and says so rather than \
leaving it implicit",
};

pub const SPRING_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx 3.6 spring_layout at dim=3 \
(networkx/drawing/layout.py:452-651), the method='force' branch _fruchterman_reingold \
(layout.py:660-727) at its own defaults (iterations=50, threshold=1e-4, scale=5.0, \
k=sqrt(1/n), d clipped to 0.01) — the call SciGraphs' _spring_layout_3d makes \
(networkx_layouts.py:26-34) and nothing else. NOT A SECOND KERNEL: this is layout.force.spring \
with the dimension as a const parameter on forces::Field and forces::Solver, in \
layout/force/spring3d.rs, over the same Solver, the same SpringParams and the same seed; the \
2D arm's hashed coordinates are pinned bit for bit by the golden the dimension change had to keep \
(layout/force/spring/tests/golden.rs), so a shared kernel is a proven fact rather than a claim. \
The difference between the two stages is the geometry and nothing else: Geometry::planar there, \
Geometry::in_space here, so this snapshot is labelled 0.4 and carries a z column. SEEDING \
DECISION, written out as for CUBE: GENERATOR is graph-core's Mulberry32 at the same fixed seed \
the 2D arm uses, SEED SOURCE is compiled in and never an environment read, and WHAT THE ORACLE \
COMPARES is NOT a coordinate gap — both arms start from different positions and diverge inside \
the first step, so a force simulation would amplify a 1-ULP difference into a different picture. \
The gate is the STRESS METRIC layout.force.spring is already held to (harness/oracle-spring.py, \
run at dim=3), each arm scored separately over 32 max-min pivots by \
crates/graph-cli/src/stress/metric.rs, gating the deficit max(0, theirs - ours) and not a \
ratio, since both correlations may be negative. NOT YET RUN AT dim=3 ON THIS TREE: what was \
measured here is the bench column (9 347.65 ms beside the 2D arm's 9 307.48 ms at 10 000 \
nodes) and an 8-seed 4-way hash. The 1000-seed dim=3 sweep is the gate's to take, which is \
why this row's ledger status reads implemented rather than gated \
(capabilities/registry/unproven.rs). Four departures from the reference are inherited \
verbatim from the 2D module and are stated there, not repeated here: the start stream, the split \
reduction, the undirected simple graph with A in {0,1}, and the n >= 500 fork where networkx's \
method='auto' switches to the L-BFGS energy minimiser this port does not reproduce. What is \
NEW at dim=3 is exactly one thing: the rescale contract of layout.py:646 now takes its `lim` \
over all THREE axes, so a z of 7 caps the drawing rather than an x of 9",
    complexity: "O(n^2) per iteration x iterations=50, so O(50 n^2) overall; the dense \
all-pairs repulsion dominates and the attraction is O(m) inside it",
    scale_ceiling: super::super::force::SPRING_CEILING,
    degradation: "past the ceiling there is no refusal and no trap either: the dense \
all-pairs repulsion still returns finite geometry, it just takes tens of seconds and keeps \
growing quadratically, so the caller must apply its own timeout. Two refusals do exist and both \
are the reference's own: a graph of fewer than 2 nodes never enters the force loop and is the \
centre in ALL THREE columns (layout.py:618-624), and a non-finite position refuses with \
StageError::NonFinite naming node.x, node.y or node.z rather than reaching the snapshot",
    ponytail: "force layouts are CHAOTIC, identically to layout.force.spring, \
layout.forceatlas2 and layout.force.barnes_hut: one added node is a different picture, not a \
perturbed one, at either dimension. The approximation that is actually named here is the \
ITERATION COUNT AND COOLING SCHEDULE: iterations is an upper bound and the loop also exits \
early once a step's total movement falls under networkx's own 1e-4 (layout.py:725-726), so two \
graphs of the same size do different amounts of work and a graph that stops early is a \
legitimate output of the reference too. DIRECTION OF THE ERROR: SILENT, not loud — a layout \
that settles sooner is still a valid layout, nothing fails, the picture is merely less relaxed, \
and the stress metric scores it as ordinary rather than as a defect. Escape hatch, from both \
ends: SpringParams::iterations is an explicit parameter shared with the 2D arm, so a caller \
lays the graph out at any budget, and the recorded hash is pinned to the default (50), which \
is what makes a change to the schedule visible as a digest change rather than as a drift. \
Ponytail (the stress metric itself): a Pearson hop/euclid correlation certifies distances and \
says nothing about orientation, so it cannot see a mirrored or rotated but otherwise \
equivalent embedding — the strongest true claim a force layout admits. Ponytail (above 500 \
nodes): networkx's method='auto' hands n >= 500 to the L-BFGS energy minimiser \
(_energy_fruchterman_reingold, layout.py:814-880) and this port does not reproduce it, so past \
that size the port and the reference are different algorithms",
};
