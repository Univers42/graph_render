//! Graphviz 16.1.0's `twopi`, the radial layout: rings around a breadth-first tree.
//!
//! Reference: `lib/twopigen/circle.c` and `lib/twopigen/twopiinit.c` of the pinned
//! Graphviz 16.1.0 release, read as an algorithm reference and reimplemented (never
//! translated, never linked — `docs/decisions/graphviz-oracle.md`). The six phases below
//! are the reference's, in its order:
//!
//! 1. `initLayout` + `findCenterNode` — every node's `nStepsToLeaf`, then the node that
//!    maximises it: the root, chosen without any attribute (twopi reads `root`, never the
//!    neato-family `start`, which is why `-Gstart` is inert for this engine).
//! 2. `setParentNodes` — a FIFO breadth-first search from that root, so `nStepsToCenter`
//!    is the tree depth and `SPARENT` its parent.
//! 3. `setSubtreeSize` — leaves counted up their own parents.
//! 4. `setSubtreeSpans` + `setPositions` — the root's `2*PI` divided among the siblings in
//!    proportion to the leaves below each, walked down the tree.
//! 5. `setAbsolutePos` — ring `r` at radius `r` inches, at the node's own angle.
//!
//! **Units are points, and that is the point.** Graphviz computes in inches and reports
//! `ND_coord = 72 * ND_pos`, so a ring's radius is exactly `72 * depth` points; this layout
//! emits points directly and does *not* rescale or recentre, unlike the networkx-ported
//! layouts beside it. Its centre is the origin, which is what the differential's
//! bounding-box rescale then removes.
//!
//! Determinism: every traversal is a loop over a CSR row or a node-index range in
//! ascending order, the two breadth-first searches are queue-ordered, and the
//! trigonometry is `libm`'s (`prompt.md` §6 D1-D10). The breadth-first searches are
//! sequential by nature — nothing about a node's place in the ring is independent of its
//! parent's — so they are not handed to a runner (D10's condition is not met, so the
//! kernel is not written in gather form; it is stated here rather than implied).
//!
//! **Neighbour order is load-bearing and is not "sorted".** The reference walks
//! `agfstedge`/`agnxtedge`, which visits a node's out-edges in creation order and *then*
//! its in-edges, skipping a self-loop on the way in; the sibling sweep that hands out
//! angles is sequential, so a different order is a different drawing.
//! [`adjacency::for_each`] is that order, and it is the only place it exists.
//!
//! **Every per-node column is allocated once, not once per component.** [`Scratch`] holds
//! the seven the per-component passes write and resets only the component's own entries
//! between components; [`adjacency::Neighbours`] and [`adjacency::Components`] are flat for
//! the same reason. Before that, an edgeless graph — one component per node — cost
//! `O(components x n)` where `registry/radial.rs` declares `O(n + m)`, and the two
//! measurements are in `docs/measurements/fix-tree-twopi.md`.
//!
//! Ponytail: a **disconnected** graph. The reference lays out each component and then
//! packs them apart with `packSubgraphs` (`twopiinit.c:118-143`); this port lays each
//! component out around the origin and leaves them overlapping. Failing input: any
//! disconnected graph. Direction: components overlap, which is cosmetic — every node
//! still lands on its own ring at a finite point. Escape hatch: connect the graph first;
//! the differential's fixtures are all connected by construction, so the two arms are
//! only ever compared where they agree.

mod adjacency;
mod angles;
mod center;
mod scratch;
mod tree;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;
use scratch::Scratch;

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.twopi";

/// The two output columns, sized once and filled in place.
///
/// Its own type rather than two `&mut Vec<f64>` so no call can hand the same column twice,
/// and so a call never passes five arguments — the house cap.
pub(super) struct Columns {
    x: Vec<f64>,
    y: Vec<f64>,
}

impl Columns {
    /// Two zeroed columns, one slot per node.
    pub(super) fn new(count: u32) -> Self {
        Self {
            x: vec![0.0; count as usize],
            y: vec![0.0; count as usize],
        }
    }

    /// The point layout's columns, narrowed once at the end.
    pub(super) fn geometry(self) -> Geometry {
        point_geometry(&self.x, &self.y)
    }

    fn set(&mut self, node: u32, x: f64, y: f64) {
        self.x[node as usize] = x;
        self.y[node as usize] = y;
    }
}

/// `POINTS_PER_INCH` (`lib/neatogen/neatosplines.c:1130`): the reference computes in
/// inches and reports points, so this is the only conversion the whole layout needs.
const POINTS_PER_INCH: f64 = 72.0;

/// `DEF_RANKSEP` (`circle.c:27`): one inch between rings, the reference's default with
/// no `ranksep` attribute set.
const RANKSEP_INCH: f64 = 1.0;

/// `UNSET` (`circle.c:28`): the sentinel `THETA` carries until a node is placed, chosen
/// outside `[0, 2*PI]` so "exactly this value" is an unambiguous "not placed".
const UNSET: f64 = 10.0;

/// `layout.twopi` at its defaults. No `Stage` impl and no `Params`, by the same decision
/// `layout::tidy_tree`, `layout::treemap` and `layout::circular` record: the reference
/// exposes `root`, `ranksep` and `overlap`, none of which the ledger row or the oracle
/// uses, and publishing a `Params` to gain a hash-gate knob would be the tail wagging
/// the dog. `GM_MUTATE_TWOPI_NODES` re-draws this stage's own model instead.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    let mut out = Columns::new(count);
    let neighbours = adjacency::Neighbours::of(topology);
    let steps = center::steps_to_leaf(&neighbours, count);
    let mut scratch = Scratch::new(count);
    for component in adjacency::components(&neighbours, count).iter() {
        let root = center::of(&steps, component);
        scratch.reset(component);
        tree::grow(&neighbours, component, root, &mut scratch);
        angles::place(&neighbours, root, &mut scratch, &mut out);
    }
    Ok(out.geometry())
}

/// Ring `depth`'s radius in points: `POINTS_PER_INCH * RANKSEP_INCH * depth`, exactly the
/// reference's `ranksep[SCENTER(n)]` after its own inch-to-point promotion.
fn radius(depth: u32) -> f64 {
    POINTS_PER_INCH * RANKSEP_INCH * f64::from(depth)
}

#[cfg(test)]
mod tests;
