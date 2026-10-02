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
pub const SPIRAL_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _spiral_layout_3d \
(SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63) at scale=5.0, reached through \
apply_graph_layout (layouts/dispatcher.py:105-106). The oracle is the ARC-LENGTH INVERSION, \
and it is the part of this file that cannot be checked as a formula: turns = max(2, \
round(sqrt(n/(0.75*pi)))) (basic.py:41, round half-to-even), omega = 2*pi*turns (:43), a \
65536-point grid = linspace(0, 1, 1<<16) (:45), speed = sqrt((0.5s)^2 + \
(0.5s*(1+grid)*omega)^2 + (2s)^2) (:46-48), length = [0, cumsum(0.5*(speed[1:] + \
speed[:-1])*step)] (:50), wanted = linspace(0, length[-1], n) (:52-55), t = interp(wanted, \
length, grid) (:56), then x = r*cos(t*omega), y = r*sin(t*omega), z = s*(2t-1), r = s*0.5*(1+t) \
(:58-63). FOUR NUMPY PRIMITIVES ARE PORTED FORMULA FOR FORMULA and each is load-bearing, \
because each has an arithmetic a naive translation gets wrong: (1) LINSPACE is start + i*step \
with the LAST ELEMENT OVERWRITTEN BY stop, so grid[65535] is exactly 1.0 and t at the last \
node is exactly 1.0 rather than 0.9999999999999986; (2) CUMSUM IS SEQUENTIAL -- length[i] \
= length[i-1] + step over 65535 additions in index order, never the pairwise or blocked \
reduction np.sum uses, so the summation order IS the value and every interpolated t inherits \
it; (3) INTERP has its own slope formula, slope = (fp[j+1]-fp[j])/(xp[j+1]-xp[j]) then \
slope*(x-xp[j]) + fp[j], both terms in that order, with j the last index whose xp[j] <= x; \
(4) ROUND IS HALF-TO-EVEN, so turns at an exact x.5 goes to the even neighbour rather than \
away from zero. (1)-(3) were each reproduced BIT FOR BIT against the pinned numpy 2.3.3 in \
ge-python-oracle -- (1) over all 65536 grid points, (3) on 200000 unrelated monotone points \
and (3)+(2) on the t column for every node count 1..64 plus 100, 101, 77, 600 and 601 -- and \
the t and z columns are pinned as IEEE-754 words in layout/basic_3d/spiral/tests.rs at n = \
1, 2 and 7, the three sizes that cover the num_nodes == 1 branch, the smallest linspace and an \
odd count. WHAT IS NOT REACHABLE, and is the reason this row's tier is `tolerance`: x and y \
are libm's sin/cos (D1) against numpy's array loops, two different implementations, which \
differ by up to 1 ulp of f64 and vanish in the f32 narrowing the motor ships. No seed is owed \
and none is published: the function draws no random number at all",
    complexity: "O(n + 2^16): a constant 65536-entry arc-length table (512 KiB) built once, \
then one binary search and three columns per node",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (the floor): the failing input is EVERY node count up to 14, because \
sqrt(n/(0.75*pi)) is below 2 there and max(2, ...) takes over (basic.py:41) -- so the first \
fourteen node counts all draw a TWO-turn spiral and the \"gap between successive turns close to \
the spacing along the curve\" the function's own docstring claims is not what happens at the \
sizes a reader is most likely to try. Direction: wrong-but-plausible, and silently so -- the \
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
