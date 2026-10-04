//! [`RANDOM_3D`] and the run shim `LAYOUTS` points at: `layout.random.3d`.
//!
//! Split out of the parent for the house line cap, and placed beside [`super::SPIRAL_3D`]
//! rather than in `registry/closed_form.rs` because the ceiling it takes is the 3-D one
//! (`super::BASIC_3D_CEILING`), not the two-column `CLOSED_FORM_CEILING` its 2-D sibling
//! takes. Same layout, two ceilings, and the difference is the third column.
//!
//! This is the one row here the SciGraphs conformance matrix does not carry: it was added
//! from `p12-t4a` under Option A (`docs/decisions/3d-ids.md`), and the other four arms of
//! that branch already had develop ids.

use super::super::{Capability, LayoutParams, Metadata};
use super::BASIC_3D_CEILING;
use crate::layout::random;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// The same refusal `super::DEGRADATION` states, written for this row alone.
///
/// **Not `super::DEGRADATION`, and the reason is blast radius rather than wording.** That
/// string is the `degradation` field of six other rows and it counts them; rewording a
/// shared constant to add a seventh would move six other capabilities' published metadata
/// to say one word more about layouts that did not change. One string, restated, beats that
/// — the rule `three_d/bipartite_3d.rs` states for itself.
const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes, and with \
SnapshotError::Length { column: \"node.z\" } if the z column does not match the node count — a \
refusal, never a wrap or a truncation. This layout refuses for no input of its own: it \
consumes a node count and nothing else, every branch of the draw is total, and an empty \
graph gets an empty drawing rather than an error";

/// `layout.random.3d`: the 2-D `layout.random` stream read at three columns.
///
/// **The oracle is a DISTRIBUTION and cannot be a coordinate.** SciGraphs'
/// `_random_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:5-9`) is
/// `np.random.RandomState(get_layout_seed()).rand(n, 3) * scale`, so its generator is
/// numpy's Mersenne Twister at a seed the dispatcher picks, and this arm's is the crate's
/// `Mulberry32` (`layout/random.rs`) at a fixed seed with no scale. No coordinate equals the
/// reference's for any seed and none ever will; what is comparable is the SHAPE, uniform on
/// `[0,1)^3`, and the differential measures exactly that — per axis, the error in the
/// sample mean against 1/2 and in the sample variance against 1/12
/// (`harness/oracle-closed-form.py`, `random_3d_gap`). `layout::random::run_seeded` is the
/// other half and is deliberately NOT this id: it draws the reference's own numbers off the
/// ported MT19937, so it is compared coordinate for coordinate and carries a seed, where
/// this one carries the registered `Mulberry32` snapshot the hash gate pins.
/// `pub(in crate::registry)`, not `pub(super)`: `three_d.rs` re-exports with `pub(super)`,
/// which from there already means the registry, and this is a child module where the same
/// word would mean `three_d` only and stop one level short.
pub(in crate::registry) const RANDOM_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _random_layout \
(SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:5-9) is np.random.RandomState(\
get_layout_seed()).rand(n, 3) * scale -- already 3D, so the reference needs no 2D port at \
all, and it is that rand(n, 3) SHAPE this arm reproduces. NOT a coordinate comparison, and \
never one: this arm's stream is the crate's Mulberry32 at a fixed seed, unscaled, so no \
coordinate equals numpy's for any seed. The differential is oracle-closed-form's \
random_3d arm (--function random_3d, harness/oracle-closed-form.py): the worst per-axis \
error in the sample mean (target 1/2) and the sample variance (target 1/12) over the gate \
seeds. The seeded arm that DOES compare coordinates is layout::random::run_seeded, which \
draws the reference's own rand(n, 3) off the ported MT19937 at the layout seed 981798123 \
and is pinned as IEEE-754 words in layout/random/tests.rs -- a separate entry point, so \
this id's registered snapshot does not move when either changes",
    complexity: "O(n): three Mulberry32 draws and three columns per node, no graph and no \
iteration",
    scale_ceiling: BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (scale_ceiling): the scale_ceiling above is INHERITED, not \
benchmarked for this layout. BASIC_3D_CEILING was measured at 1 000 000 nodes against \
sphere, helix, cube and layout.hierarchical3d (three_d.rs:28-42), and its memory arms \
cover six rows of the seven now under it; layout.random.3d is the seventh and has neither a \
timing nor a memory arm of its own. The extrapolation is defensible on the OUTPUT side -- \
it is O(n) in three columns with no graph and no iteration, exactly like sphere and helix, \
and the substrate is what binds every sibling -- and NOT on the input side, which is where \
the difference is: this arm holds three live columns at once where the 2-D arm holds two, \
so peak is 1.5x the sibling's per node before the snapshot's own three columns are counted, \
and no one has run it at any size. Failing input: any size at all, because the figure is \
inherited rather than measured. Direction: slow and memory-hungry, never wrong. Escape \
hatch: raise it, then run `graph-cli bench --layout layout.random.3d --n 1000000` and add \
the row to crates/graph-core/tests/memory/three_d.rs. Ponytail (the metric): the oracle is \
a distribution, so the SMALLEST graph in the sweep sets the floor rather than our \
arithmetic. The expected |sample mean - 1/2| for two uniform draws is 0.204 and the \
measured worst at n = 2 is 0.3030, the same order; it falls as 1/sqrt(n) (0.0384 over the \
cases with n >= 100), which is the signature of sampling noise and not of a skewed stream. \
Direction: wrong-but-plausible, and silently so -- a stream that leaned towards one end of \
[0,1) would still draw points in the unit cube and nothing in the picture would show it. \
Escape hatch: the per-axis mean and variance are the two numbers, and raising the seed count \
lowers the floor. Ponytail (the scale): the reference multiplies by the dispatcher's \
scale = 5.0 and this arm does not, so the drawing is a unit cube where the reference's fills \
a 5x5x5 box; the oracle is scale-free for that reason and the geometry is not \
interchangeable with the reference's. Direction: none visible, a random layout has no \
correct answer. Escape hatch: layout::random::run_seeded, which is the scaled arm",
};

/// `LAYOUTS` appends this after `layout.force.fa2.3d`, in this order (append only: the wasm
/// module maps a layout by index).
pub(in crate::registry) const RANDOM_3D_LAYOUT: Capability = Capability {
    id: random::ID_3D,
    run: random::run_3d,
    params: &LayoutParams::NONE,
    meta: RANDOM_3D,
};
