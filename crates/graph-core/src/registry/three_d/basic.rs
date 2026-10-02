//! The three graph-free 3D closed forms: [`SPHERE`], [`HELIX`] and [`CUBE`]. Split out of
//! the parent for the house line cap; the ceiling and the degradation string they all share
//! stay there.

use super::super::Metadata;
use super::{BASIC_3D_CEILING, DEGRADATION};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

pub const SPHERE: Metadata = Metadata {
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

pub const HELIX: Metadata = Metadata {
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

pub const CUBE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _cube_layout (SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:83-103) \
at scale=5.0. SEEDING DECISION, written out because this is the one of the three that draws \
from a stream. GENERATOR: graph-core's own Mulberry32 at a fixed compiled-in seed, the same \
stream layout.random and the force layouts use (D5: the motor has no global RNG and the same \
graph must hash the same on every target). SEED SOURCE: compiled in, never an environment read, \
never a cfg. WHAT THE ORACLE COMPARES: the corners exactly (all eight, in all eight slots, \
because their ORDER is the layout — basic.py:91-94 is a literal array and a reordering is a \
visible regression, not a refactor), the min(n, 8) split, the n == 1 origin (basic.py:88-89), and \
the interior's DISTRIBUTION but never its coordinates: uniform on [-0.8*scale, 0.8*scale] per \
axis, mean 0, variance (1.6*scale)^2/12, which is what harness/oracle-basic-3d.py --function cube \
measures. WHY THE STREAM IS NOT THE REFERENCE'S, in two parts, both making a \
coordinate-for-coordinate port impossible rather than merely hard: (1) the reference draws from \
numpy's RandomState, not mulberry32, so NO interior coordinate equals SciGraphs' for any seed; \
(2) _get_layout_rng() (common.py:43-52) returns a MODULE-LEVEL GLOBAL RandomState, so the \
reference's interior depends on every earlier layout in the process that drew from it — there \
is no 'the' interior to compare against, only the first call after a reset. This is the same \
answer registry/closed_form.rs:28-30 gives for layout.random, and for the same reason",
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
+scale). Ponytail (interior stream): the interior's coordinates are ours and not the \
reference's (see oracle), so failing input is any comparison of a node past 8 against \
SciGraphs' own, which will disagree and is meant to; direction is a different picture inside \
the same shell, which is invisible; escape hatch is the distribution the differential does \
gate, and the corners below 9 nodes, which it gates exactly",
};
