//! `layout.bipartite_3d`: two node sets on parallel planes, one ring each. SciGraphs'
//! `_bipartite_layout_3d`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:213-242`), ported formula
//! for formula at [`SCALE`].
//!
//! The construction, as the reference writes it:
//!
//! ```text
//! parts  = _bipartite_parts(G), or _greedy_max_cut(G)     which two sets
//! for nodes, z in ((set0, -scale*0.5), (set1, +scale*0.5)):
//!     count  = len(nodes)                                 one ring's node count
//!     angle  = (i / max(1, count)) * 2 * pi               i is the slot in the SET
//!     radius = scale * 0.6                                one radius for both planes
//!     x      = radius*cos(angle)
//!     y      = radius*sin(angle)
//! ```
//!
//! **This is not `layout.bipartite`, and the two must not be confused.** That layout is
//! networkx's `bipartite_layout`: two vertical **columns** in the plane, rescaled to a unit
//! box. This one is two horizontal **rings** at `z = ±scale*0.5` and is not rescaled at
//! all. They share the two node sets and nothing else, which is why they share
//! `layout::bipartite`'s partition and why neither one's oracle covers the other's
//! placement.
//!
//! **The sets are reused from [`crate::layout::bipartite`] rather than re-ported, and the
//! reason is that its visiting order is already the reference's, node for node.** Both walk
//! `adjacency::neighbours`, which is networkx's own order; `_bipartite_parts` (`hierarchical.py:149`)
//! iterates `G.nodes()` in insertion order and BFSs each component from the first uncoloured
//! node with that node on colour 0, flipping a component only when
//! `abs(skew + len0 - len1) > abs(skew + len1 - len0)` (`:177`) — a strict `>`, so a tie keeps
//! the start side. `_greedy_max_cut` (`:183`) places a node on side 1 exactly when
//! `placed[0] > placed[1]` (`:194`) and then runs at most 8 single-vertex passes
//! (`:196-207`). `partition.rs` is those two rules, in that order, with those tie-breaks.
//! Reuse is therefore exact and is recorded here so the next reader does not port it twice.
//!
//! **The `max(1, count)` guard is a division the reference never performs** (`hierarchical.py:237`):
//! `i` enumerates `nodes`, so `count >= 1` on every iteration and the clamp is dead. It is
//! written out as the plain ratio rather than transcribed, because a transcribed
//! `max(1, count)` would be unreachable code pretending to be a guard.

use super::{SCALE, in_space};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::bipartite;
use crate::stage::StageError;

/// The fixed radius ratio, `scale * 0.6` (`hierarchical.py:238`). Not a parameter: the
/// reference has one `scale` and this coefficient is part of the shape, not of its tuning —
/// and it is the same radius on **both** planes, so the two rings are congruent.
const RADIUS_RATIO: f64 = 0.6;

/// The plane offset ratio, `scale * 0.5` (`hierarchical.py:234`), negated for the first set.
/// The full plane separation is therefore exactly `scale`, and `z` is the one coordinate
/// here that is not a transcendental of anything.
const PLANE_RATIO: f64 = 0.5;

/// `BIPARTITE_3D`'s capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.bipartite_3d";

/// `_bipartite_layout_3d(G, scale)` (`hierarchical.py:213-242`). Never refuses.
pub(super) fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let (x, y, z) = columns(topology);
    Ok(in_space(&x, &y, &z))
}

/// The three `f64` columns, before the single narrowing in [`in_space`].
///
/// Gather form (D10): every column is written at `node`'s own index, so the drawing does not
/// depend on the order the two sets were placed in — only the sets themselves are ordered.
pub(super) fn columns(topology: &Topology) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (lower, upper) = bipartite::node_sets(topology);
    let zeroed = || vec![0.0; topology.node_count() as usize];
    let mut columns = (zeroed(), zeroed(), zeroed());
    for (nodes, sign) in [(&lower, -1.0), (&upper, 1.0)] {
        ring(&mut columns, nodes, sign * SCALE * PLANE_RATIO);
    }
    columns
}

/// One set on its own plane: every node at `z`, spread evenly in angle over a full turn.
///
/// `nodes` is the set in SciGraphs' own order and `slot` is the node's index **in that
/// order**, not its dense index — the two differ on every node the colouring moved, and using
/// the dense index is the mistake that draws the same ring in a different order.
fn ring(columns: &mut (Vec<f64>, Vec<f64>, Vec<f64>), nodes: &[u32], z: f64) {
    let count = nodes.len();
    let radius = SCALE * RADIUS_RATIO;
    for (slot, &node) in nodes.iter().enumerate() {
        let angle = angle(slot, count);
        let at = node as usize;
        columns.0[at] = radius * libm::cos(angle);
        columns.1[at] = radius * libm::sin(angle);
        columns.2[at] = z;
    }
}

/// `(i / max(1, count)) * 2 * pi` (`hierarchical.py:237`), in the reference's own order.
///
/// **Left-associative, and it has to be.** The reference writes `(i / max(1, count)) * 2 * np.pi`,
/// which is `((i / count) * 2) * pi` and **not** `(i / count) * (2 * pi)`. The two differ in
/// the last ulp, and the difference survives into `f32` for some nodes. Written the other
/// way round — one multiply by a precomputed `TAU` — the ring is a hair larger at a hair
/// different angle, and every formula in it still reads correctly.
fn angle(slot: usize, count: usize) -> f64 {
    (slot as f64 / count as f64) * 2.0 * core::f64::consts::PI
}

#[cfg(test)]
mod tests;
