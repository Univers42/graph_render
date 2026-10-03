//! [`SPIRAL_3D`]: SciGraphs' conical spiral, the one layout of these whose oracle is a
//! formula over an arc-length inversion. Split out of the parent for the house line cap.

use super::super::Metadata;
use super::{BASIC_3D_CEILING, DEGRADATION};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// `SPIRAL_3D`: SciGraphs' conical spiral, and the one of these layouts whose oracle is
/// **not** a formula but a formula over an arc-length inversion.
///
/// **This is not `layout.spiral`'s oracle.** That id is networkx's planar
/// `spiral_layout` at `resolution = 0.35`, which SciGraphs never calls — SciGraphs has no
/// 2D spiral, and `SPIRAL_3D` reaches `_spiral_layout_3d` (`basic.py:36-63`) through
/// `dispatcher.py:105` and nothing else. The conformance doc's earlier repair ("raise
/// `resolution` to 1.0") was wrong on both counts and is corrected in
/// `docs/measurements/sg-spiral3d.md`.
///
/// **`ARMS` in `harness/oracle-basic-3d.py` and `oracle_python/basic_3d.rs` now cover this
/// layout too** — job `sg-basic3d-spiral-oracle` added `--function spiral`, measured worst
/// `2.384e-7` against a `1e-6` ceiling over 1000 seeds, bit-identical on all 1000 after the
/// `f32` narrowing. This row is **not yet routed to that record**: `unproven.rs` still sends
/// it to `scigraphs-conformance`, and moving the routing is the half of that job left over.
///
/// The comparisons that exist today are the conformance gate (`scripts/scigraphs-conformance.sh`,
/// byte-for-byte against SciGraphs over 1020 coordinates, `f32` 1020/1020) and the
/// graph-core tests, which pin the reference's own IEEE-754 words at n = 1, 2 and 7.
pub const SPIRAL_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _spiral_layout_3d \
(SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63) at scale=5.0, reached through \
apply_graph_layout (layouts/dispatcher.py:105-106). The oracle is the ARC-LENGTH INVERSION, \
and it is the part of this file that cannot be checked as a formula: turns = max(2, \
round(sqrt(n/(0.75*pi)))) (basic.py:41, round half-to-even; the max(2,..) floor lifts the \
value at exactly n = 1..5, since the raw round is already 2 from n = 6 to 14), omega = \
2*pi*turns (:43), a \
65536-point grid = linspace(0, 1, 1<<16) (:45), speed = sqrt((0.5s)^2 + \
(0.5s*(1+grid)*omega)^2 + (2s)^2) (:46-48), length = [0, cumsum(0.5*(speed[1:] + \
speed[:-1])*step)] (:50), wanted = linspace(0, length[-1], n) (:52-55), t = interp(wanted, \
length, grid) (:56), then x = r*cos(t*omega), y = r*sin(t*omega), z = s*(2t-1), r = s*0.5*(1+t) \
(:58-63). FOUR NUMPY PRIMITIVES ARE PORTED FORMULA FOR FORMULA and each is load-bearing, \
because each has an arithmetic a naive translation gets wrong: (1) LINSPACE is start + i*step \
(the reference also overwrites the last element with stop, but 65535*step is ALREADY exactly \
1.0 -- measured, 0x3ff0000000000000 -- so there is no last-element special case to port and \
grid_at carries none); (2) CUMSUM IS SEQUENTIAL -- length[i] \
= length[i-1] + step over 65535 additions in index order, never the pairwise or blocked \
reduction np.sum uses, so the summation order IS the value and every interpolated t inherits \
it; (3) INTERP has its own slope formula, slope = (fp[j+1]-fp[j])/(xp[j+1]-xp[j]) then \
slope*(x-xp[j]) + fp[j], both terms in that order, with j the last index whose xp[j] <= x; \
(4) ROUND IS HALF-TO-EVEN, so turns at an exact x.5 goes to the even neighbour rather than \
away from zero. (1)-(3) were each reproduced BIT FOR BIT against the pinned numpy 2.3.3 in \
ge-python-oracle -- (1) over all 65536 grid points, (3) on 200000 unrelated monotone points \
and (3)+(2) on the t column for 69 node counts, 1..64 plus 77, 100, 101, 600 and 601 -- and \
the t and z columns are pinned as IEEE-754 words in layout/basic_3d/spiral/tests/ at n = \
1, 2 and 7, the three sizes that cover the num_nodes == 1 branch, the smallest linspace and an \
odd count. DIVERGENCE AT n = 0, and it is the REFERENCE that refuses: basic.py:52's guard is \
`if num_nodes > 1`, so num_nodes = 0 falls into the num_nodes == 1 branch, np.column_stack \
returns shape (1, 3) for a graph with no nodes, and _check_positions \
(layouts/common.py:175-185) raises ValueError so apply_graph_layout returns False (measured in \
ge-python-oracle). This port returns an EMPTY 3D geometry instead, deliberately: run is \
Result<Geometry, StageError> and every function in basic_3d is total, so an empty graph gets an \
empty drawing on all four ids rather than an error on one of them. Unreachable from the \
conformance matrix, whose smallest fixture has 2 nodes. WHAT IS NOT REACHABLE, and is the \
reason this row's tier is `tolerance`: x and y \
are libm's sin/cos (D1) against numpy's array loops, two different implementations, which \
differ by up to 1 ulp of f64 and vanish in the f32 narrowing the motor ships. No seed is owed \
and none is published: the function draws no random number at all",
    complexity: "O(n + 2^16): a constant 65536-entry arc-length table (512 KiB) built once \
per call -- 65535 sqrt and 65535 sequential additions, independent of n -- then one binary \
search and three columns per node",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (UNMEASURED ceiling): the scale_ceiling above is INHERITED, not \
benchmarked for this layout. BASIC_3D_CEILING was measured at 1 000 000 nodes against \
sphere, helix, cube and hierarchical3d (three_d.rs:28-42); layout.basic3d.spiral was never \
run at that size or any other by `graph-cli bench`, so the figure is an extrapolation from its \
siblings. The extrapolation is defensible on the OUTPUT side -- it is O(n) in three columns \
with no graph and no iteration, exactly like sphere and helix -- and NOT on the input side, \
which is where the real difference is: every call builds a fixed 65 536-entry arc-length \
table, so the constant cost is 65 535 sqrt and 65 535 sequential additions regardless of n, \
and that is what an unmeasured 1 M-node figure has never paid. Failing input: any size, at \
the constant. Direction: slow and memory-hungry, never wrong. Escape hatch: none, and \
deliberately so -- a closed-form conic arc-length integral would stop agreeing with \
SciGraphs' own grid and is therefore a different layout id, not a faster build of this one. \
Nothing at or past 1 M nodes has been run for this layout, so the number understates the \
wall by an unmeasured amount. Ponytail (the floor): the failing input is n = 1..5, where \
max(2, ...) is what lifts the turn count (basic.py:41) -- the raw round is 1 there and \
already 2 from n = 6 to 14, so at n = 7, the size a reader reaches for first, the floor \
changes nothing. Direction: wrong-but-plausible, and silently so -- the \
spiral is still a spiral, every node is still exactly on the reference's cone, and nothing in \
the picture shows that the turn count was floored. Escape hatch: layout/basic_3d/spiral.rs's \
turns(), which is one expression. Ponytail (the single node): n == 1 takes basic.py:52-55's \
guard and asks for HALF the arc length, not the whole of it -- but the climb is linear in t \
and not in arc length, so half the arc is t = 0.5775 and NOT the midpoint z = 0 a reader would \
guess; the one-node drawing sits at r = 3.75, z = +0.775, and that number is pinned in the \
tests because it is the one no shortcut reproduces. Ponytail (the table): 512 KiB and 65535 \
sequential additions per call regardless of n, for a layout whose output is O(n) -- a \
reference-faithful cost, not a necessary one; the failing input is any caller running this at \
n = 2 in a loop. Direction: slow and memory-hungry, never wrong. Escape hatch: none, and \
deliberately so -- the alternative is a closed-form conic arc-length integral, which would \
stop agreeing with SciGraphs' own grid and is therefore a different layout id, not a faster \
build of this one",
};
