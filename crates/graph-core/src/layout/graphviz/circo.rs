//! Graphviz 16.1.0's `circo`, the circular layout of Six and Tollis.
//!
//! Reference: `lib/circogen` of the pinned Graphviz 16.1.0 release — `circularinit.c`,
//! `circular.c`, `blocktree.c`, `blockpath.c` and `circpos.c` — read as an algorithm
//! reference and reimplemented. Never translated line by line, never linked
//! (`docs/decisions/graphviz-oracle.md`). The five phases are the reference's, in its order:
//!
//! 1. **The derived graph** (`graph`): strict and undirected, so self-loops are not edges
//!    and both directions of a pair are one edge.
//! 2. **Blocks** (`blocks`): one lowlink walk from each component's first node finds the
//!    biconnected components and hangs them on a block-cutpoint tree.
//! 3. **The skeleton and the circle order** (`skeleton`, `circle`): per block, thin the
//!    block to a skeleton, read its long path, fill in the rest, reduce crossings, and place
//!    node `k` of `n` at angle `2*PI*k/n`.
//! 4. **The placement** (`position`): hang each block's child circles around it, bottom-up.
//!
//! **Units are inches here and points in the snapshot.** The reference computes in inches
//! (`ND_width` is 0.75, `mindist` is 1.0 — `circular.c:18`, `utils.c:431`) and its
//! postprocess multiplies by 72 (`ND_coord`). This layout therefore promotes once, at the
//! end, exactly as the reference does.
//!
//! Determinism: every traversal is a loop over a node's own `agfstedge`-ordered row or an
//! ascending index range, the two lowlink frames and the spanning-tree frames are explicit
//! stacks, and the trigonometry is `libm`'s (`prompt.md` §6 D1-D10). Nothing here iterates a
//! hash map: `graph::BlockGraph` keeps one for edge lookup only. The placement is
//! sequential by nature — a child circle's position depends on its parent's — so it is not
//! handed to a runner, and that is stated rather than implied (D10's condition is not met, so
//! the kernel is not written in gather form).
//!
//! Ponytail: a **disconnected** graph. The reference lays each component out and then packs
//! them apart with `packSubgraphs` (`circularinit.c:205-222`); this port lays each component
//! out around the origin and leaves them overlapping. Failing input: any graph with two
//! components. Direction: overlap, the cosmetic one — every node still lands at a finite
//! point inside its own block's circle, and no edge is mis-drawn. Escape hatch: connect the
//! graph first; the differential's fixtures are connected by construction, so the two arms
//! are only ever compared where they agree.

mod blocks;
mod circle;
mod crossings;
mod graph;
mod position;
mod rotation;
mod skeleton;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;

use blocks::Found;
use graph::{BlockGraph, Derived};

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.circular.circo";

/// `MINDIST` (`circular.c:18`): the gap a circle leaves between two of its nodes, in inches.
pub(super) const MIN_DIST: f64 = 1.0;

/// The full turn, as `2 * M_PI`.
pub(super) const TAU: f64 = 2.0 * std::f64::consts::PI;

/// `POINTS_PER_INCH`: the reference's inch-to-point promotion, done once, at the end.
const POINTS_PER_INCH: f64 = 72.0;

/// `largest_nodesize` (`blockpath.c:495-507`) over a default node: `max(ND_width,
/// ND_height)` is `max(0.75, 0.5)`, and every node in a `-Tplain` fixture is a default one, so
/// the fold over the block always lands here. It is load-bearing: it is most of the circle's
/// radius. Kept in [`circle`] with the radius formula it belongs to.
pub(super) const NODE_SIZE_INCH: (f64, f64) = (0.75, 0.5);

/// One block of the block-cutpoint tree, and the state the placement fills in.
pub(super) struct Block {
    /// The block's nodes in `agsubnode` order — the order the reference's pops inserted them,
    /// which is not ascending, and which several sweeps walk.
    pub(super) nodes: Vec<u32>,
    /// Child blocks, in the order `createBlocktree` hung them on.
    pub(super) children: Vec<usize>,
    /// The parent block, or `None` for a component's root block.
    pub(super) parent: Option<usize>,
    /// `CHILD(b)` (`block.h:50`): the node **in this block** that names its parent.
    pub(super) child_node: u32,
    /// `BLK_PARENT(b)` (`block.h:51`): the node **in the parent block** this one hangs off.
    pub(super) hangs_at: u32,
    /// The block's radius, grown as children are attached.
    pub(super) radius: f64,
    /// `rad0`: the radius before any child was attached.
    pub(super) rad0: f64,
    /// Where the parent sits, for a one-node block (`-1` when unused).
    pub(super) parent_pos: f64,
    /// Whether this block was coalesced with its single child.
    pub(super) coalesced: bool,
    /// The block's nodes in circle order, as indices into [`Block::nodes`].
    pub(super) circle: Vec<u32>,
}

impl Block {
    /// A block with nothing in it yet.
    pub(super) fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            children: Vec::new(),
            parent: None,
            child_node: 0,
            hangs_at: u32::MAX,
            radius: 0.0,
            rad0: 0.0,
            parent_pos: -1.0,
            coalesced: false,
            circle: Vec::new(),
        }
    }
}

/// Everything the placement reads and writes, in one place so no phase needs five arguments.
pub(super) struct Layout {
    pub(super) blocks: Vec<Block>,
    /// One root block per connected component with more than one node.
    pub(super) roots: Vec<usize>,
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    /// `PSI(n)`: the mid-angle of a node's children, consumed by `getRotation`.
    pub(super) psi: Vec<f64>,
    /// `PARENT_F` per node.
    pub(super) parent_flag: Vec<bool>,
}

impl Layout {
    fn of(found: Found, count: u32) -> Self {
        Self {
            blocks: found.blocks,
            roots: found.roots,
            x: vec![0.0; count as usize],
            y: vec![0.0; count as usize],
            psi: vec![0.0; count as usize],
            parent_flag: found.parent_flag,
        }
    }

    /// `ISPARENT(n)`.
    pub(super) fn is_parent(&self, node: u32) -> bool {
        self.parent_flag[node as usize]
    }

    /// `layout_block`'s order for `at`: the long path, the residual pass, the crossing
    /// reduction, and the one rotation that puts the block's `PARENT_F` node first.
    fn circle_of(&self, derived: &Derived, at: usize) -> Vec<u32> {
        let graph = BlockGraph::of(derived, &self.blocks[at]);
        let mut order = skeleton::order_of(&graph);
        order = circle::order_of(&graph, order);
        let nodes = &self.blocks[at].nodes;
        if let Some(at) = order
            .iter()
            .position(|&local| self.parent_flag[nodes[local as usize] as usize])
        {
            circle::realign(&mut order, at);
        }
        order
    }
}

/// `layout.circular.circo` at its defaults. No `Stage` impl and no `Params`, by the same
/// decision `layout::tidy_tree` and `layout.twopi` record: the reference exposes `mindist`,
/// `root` and `oneblock`, none of which the ledger row or the oracle uses, and publishing a
/// `Params` to gain a hash-gate knob would be the tail wagging the dog.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    let derived = Derived::of(topology);
    let mut layout = Layout::of(blocks::decompose(&derived, count), count);
    for root in layout.roots.clone() {
        position::place(&mut layout, &derived, root);
    }
    let x: Vec<f64> = layout.x.iter().map(|v| v * POINTS_PER_INCH).collect();
    let y: Vec<f64> = layout.y.iter().map(|v| v * POINTS_PER_INCH).collect();
    Ok(point_geometry(&x, &y))
}

#[cfg(test)]
mod tests;
