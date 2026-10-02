//! Ledger metadata for the last five SciGraphs layouts, the five that are natively 3D and
//! were the last `missing` rows in `docs/measurements/scigraphs-coverage.md`.
//!
//! Kept apart from `registry.rs` for the house line cap, and because these five are one
//! subject: they carry a z column, so a snapshot of any of them is labelled 0.4 rather
//! than 0.3 (`docs/decisions/contract-3d-verdict.md`, condition 1) and no recorded 2D
//! digest moves.
//!
//! **They are two kinds of thing, and the metadata says which per row.** [`SPHERE`],
//! [`HELIX`] and [`CUBE`] are closed forms over `(num_nodes, scale)` that read no graph at
//! all; [`HIERARCHICAL_3D`] reads the graph and [`SPRING_3D`] iterates. The three closed
//! forms owe no seed and say so; the two that draw or read structure say what they compare.

mod bipartite_3d;

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

pub(super) use bipartite_3d::BIPARTITE_3D;

/// The node count the three graph-free 3D placements were run at, and why it is this one.
///
/// `graph-cli bench` refuses a size past a layout's registered `scale_ceiling`, and its own
/// cap is 1 000 000 nodes (`bench/scale.rs:33-34`, ten components of 100 000), so 1 M is
/// the largest size a measurement of any layout in this file can be taken at. It is a
/// **measured lower bound, not the wall**: all three layouts are `O(n)` in three `f64`
/// columns with no graph and no iteration, so what binds at 1 M nodes is the 48 bytes a
/// node costs in the snapshot's own three columns plus the 32 in the geometry's, not the
/// layout — and wasm32's 4 GiB would put the true wall several times higher.
///
/// Measured, `--release`, `--repeat 3`, medians, at 1 000 000 nodes and 1 549 929 edges on
/// this host:
///
/// ```sh
/// scripts/orch/gr cargo run -q --release -p graph-cli -- bench \
///   --layout layout.basic3d.sphere,layout.basic3d.helix,layout.basic3d.cube,layout.hierarchical3d \
///   --n 1000000 --repeat 3
/// ```
///
/// ```text
/// n=1000000
///   layout.basic3d.sphere           57.44 ms  stress-1 0.4531  (edges=1549929)
///   layout.basic3d.helix            12.69 ms  stress-1 0.4605  (edges=1549929)
///   layout.basic3d.cube              4.77 ms  stress-1 0.4836  (edges=1549929)
///   layout.hierarchical3d          306.94 ms  stress-1 0.4259  (edges=1549929)
/// ```
///
/// `layout.force.spring3d` is the exception and takes [`SPRING_CEILING`] instead: it is the
/// dense `O(50 n^2)` kernel, not a closed form. Measured beside its 2D sibling at the same
/// two sizes — `bench --layout layout.force.spring,layout.force.spring3d --n 10000,16000
/// --repeat 3`: 9 307.48 ms against 9 347.65 ms at 10 000 nodes, and 23 261.70 ms against
/// 23 443.23 ms at 16 000 — so the third column costs about 0.8% rather than 50%, and the
/// 2D arm's ceiling stands for both.
///
/// Ponytail (scale_ceiling): every digit here is one host's median at one size; another host
/// moves every one, and the bracketing is what carries the claim rather than the digits.
/// Nothing above 1 000 000 nodes was run, so the figure understates the wall.
pub const BASIC_3D_CEILING: u64 = 1_000_000;

/// The z-bearing degradation string, in the shape `registry/closed_form.rs:18` gives: what
/// happens past the ceiling on wasm32, what happens natively, and the explicit promise
/// that it is a refusal — never a wrap, never a truncation.
///
/// **The 3D addition is one clause and it is not optional.** Past the ceiling natively the
/// snapshot can also fail to build its z column, and the wire name it fails under is
/// `node.z` (`graph-contract/src/geometry/columns.rs:14-37`) — the same refusal a
/// non-finite x would get, at a different column.
const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes, and with \
SnapshotError::Length { column: \"node.z\" } if the z column does not match the node count — a \
refusal, never a wrap or a truncation. None of these five layouts refuses for any input of \
its own: every branch of each reference function is total, so there is no graph past which \
this module's own answer stops existing";

pub(super) const SPHERE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _sphere_layout (SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-34), \
ported formula for formula at the dispatcher's own scale=5.0 (dispatcher.py:14). The oracle is the \
FORMULA, not the shape, and the two constants it turns on are both load-bearing: y = 1 - \
2*(i+0.5)/n (basic.py:29), the band MIDPOINT rather than the band edge, which is what keeps the \
two poles from over-packing (the function's own docstring cites Gonzalez 2010 and Marques et al. \
2013), and theta = pi*(3 - sqrt(5))*i (basic.py:32), the golden angle 2*pi/phi computed once and \
multiplied by the node's index, never accumulated. Axis order is the reference's and is not \
interchangeable: np.column_stack puts cos(theta)*r on x, y on y, and sin(theta)*r on z \
(basic.py:34). Compared against the SciGraphs arm itself in the ge-python-oracle image with the \
submodule mounted, by harness/oracle-basic-3d.py --function sphere, as coordinates within \
1e-6 — the next power of ten above the f32 narrowing both arms share. No seed is owed and none \
is published: the function draws no random number at all",
    complexity: "O(n): three libm transcendentals and one division per node, no graph and no \
iteration",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the failing input is a handful of nodes — at n = 2 and n = 3 the \
Fibonacci construction puts its nodes at nearly equal latitudes and the drawing reads as a \
straight line of two or three points rather than a sphere; at n < 8 no two nodes share a \
latitude band comfortably. Direction: COSMETIC and structural, never wrong-but-plausible \
spacing: every node is still exactly on the shell of radius scale, which is asserted over \
sizes 1 to 257. What it is not: this is a unit sphere SCALED, so it is a set of points on a \
SURFACE, not a surface-density-uniform point set — it is empty in the middle by \
construction, and a caller wanting a filled ball wants another layout id. Escape hatch: \
another layout id, and the scale constant is the only knob (there is no radius or \
thickness parameter to reach for, and adding one would be a layout SciGraphs does not ship)",
};

pub(super) const HELIX: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _helix_layout (SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:65-81), \
ported formula for formula at scale=5.0. The oracle is the formula and its one branch: levels = \
(n+1)//2 (basic.py:68), t = (i // 2) / (levels - 1) (basic.py:72), angle = t*4*pi + (i % 2)*pi \
(basic.py:76), radius = scale*0.3 (basic.py:77), z = t*scale*2 - scale (basic.py:81). The \
levels == 1 branch (basic.py:71-74) returns t = 0.5 for every node and is compared there \
explicitly, at n = 1 AND n = 2, because that is where a port silently disagrees: a return of \
t = 0 has every formula still reading correctly and draws the helix at its foot instead of its \
midpoint. An odd node count leaves the extra node on strand 0, because (i % 2) is the strand \
and nothing rounds to make the strands even. Compared against the SciGraphs arm itself by \
harness/oracle-basic-3d.py --function helix, as coordinates within 1e-6, over node counts 1 \
to 601 inclusive so both sides of the levels > 1 boundary are in the sweep. No seed is owed \
and none is published",
    complexity: "O(n): two libm transcendentals per node, no graph and no iteration",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the failing input is n = 1 and n = 2 — both take the levels == 1 \
branch, so the 'helix' is one or two points at z = 0 and there is no helix to see at all. \
Direction: COSMETIC and loud rather than silent: at n <= 2 the drawing is degenerate by \
construction and every node is at the midpoint, so nothing plausible-but-wrong can be read \
into it; at n >= 4 it is two full turns. The approximation that is real is not the \
degenerate case but the shape itself: the radius is a CONSTANT scale*0.3, not a taper, and \
t is uniform in index rather than uniform in arc length, so the nodes are evenly spaced in \
the parameter t and not along the curve — the drawing is a parameterised helix, not an \
arc-length-even one. Escape hatch: another layout id; SciGraphs ships no radius, taper or \
pitch parameter and neither does this port",
};

pub(super) const CUBE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _cube_layout (SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:83-103) \
at scale=5.0. GENERATOR: the reference's own, ported exactly — numpy legacy RandomState, i.e. \
MT19937 seeded by init_genrand, at derive_seed(42, 'layout') = 981798123, reproduced by \
crate::rng::Mt19937. WHY IT IS THE REFERENCE'S GENERATOR: the rows are compared coordinate for \
coordinate, and a different generator at the same seed draws a scatter that is uniformly \
distributed in the same shell and shares not one coordinate with the reference. SEED SOURCE: \
compiled in, never an environment read, never a cfg. THE STREAM IS RESET PER CALL, not carried \
across calls: apply_graph_layout calls _reset_layout_rng() on entry (dispatcher.py:22), which \
rebuilds the module-global as a fresh np.random.RandomState(get_layout_seed()) (common.py:53-60), \
so every layout draws from the start of a fresh stream and the interior is reproducible for \
every call — an earlier version of this metadata claimed only the first call after a reset was \
reproducible, and the reference does not behave that way. The corners draw nothing, so the \
interior is the stream's first 3*(n - min(n,8)) values, C order, x then y then z per node. WHAT \
THE ORACLE COMPARES, all of it exactly: the eight corners in all eight slots, because their \
ORDER is the layout — basic.py:91-94 is a literal array and a reordering is a visible \
regression, not a refactor — the min(n, 8) split, the n == 1 origin (basic.py:88-89), and every \
interior coordinate, as (-1.0 + 2.0*u) * (scale*0.8) with u the reference's own draw. The \
interior's DISTRIBUTION (uniform on [-0.8*scale, 0.8*scale] per axis) remains what \
harness/oracle-basic-3d.py --function cube measures, and it is now a consequence rather than a \
substitute",
    complexity: "O(n): eight corners by table lookup, then three stream draws and three \
multiplies per remaining node",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (corner order): the failing input is any node count over 7, because \
the first 8 nodes' positions ARE the reference's literal array order (basic.py:91-94) — a \
reordering of that array is a VISIBLE REGRESSION, not a refactor: the drawing is the same cube \
with different nodes at its corners, so nothing in the picture would show it while every \
hashed snapshot moves. Direction: wrong-but-plausible — every node is still exactly on a cube \
corner. Escape hatch: the array is one `const CORNERS` in layout/basic_3d/cube.rs, and \
crate::graph_core::layout::basic_3d::CORNERS is public, so a caller can read the order rather \
than infer it. Ponytail (single node): n == 1 returns the ORIGIN (basic.py:88-89), not a \
corner, so a one-node cube is a point at the centre and the cube's shell is entirely absent — \
checked before the corners are built, because min(1, 8) would have put it at (+scale, +scale, \
+scale). Ponytail (interior stream): the interior's coordinates ARE the reference's, so the \
failing input is a generator other than numpy's legacy RandomState — a correct-looking \
uniform scatter that shares no coordinate with SciGraphs, which is invisible in the picture and \
total in the bytes; direction is a different picture inside the same shell. Escape hatch is \
layout.random's Mulberry32 default and the corners, which are a closed form and need no stream",
};

pub(super) const HIERARCHICAL_3D: Metadata = Metadata {
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

pub(super) const SPRING_3D: Metadata = Metadata {
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
    scale_ceiling: super::force::SPRING_CEILING,
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
