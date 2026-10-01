//! The breadth-first tree from the root: depths, parents, child counts and leaf counts.
//!
//! Reference `setParentNodes` (`circle.c:147-166`), `setNStepsToCenter`
//! (`circle.c:117-140`) and `setSubtreeSize` (`circle.c:172-182`).
//!
//! The reference's search is a FIFO queue already (`LIST_PUSH_BACK` then
//! `LIST_POP_FRONT`) and relaxes on strictly smaller distance, so a node is discovered
//! exactly once and `NCHILD(n)` is a true child count. The port keeps the queue and the
//! strict `<`; it differs from the reference only in refusing to enqueue a node whose
//! parent is unset — the reference's `setParentNodes` reports `UINT64_MAX` there and the
//! engine abandons the drawing, which is the disconnected case recorded as a Ponytail in
//! `super`.

use super::adjacency::Neighbours;

/// No parent: the root, and every node outside this component.
pub(super) const NONE: u32 = u32::MAX;

/// The tree, and the per-node columns the angle sweep reads.
pub(super) struct Tree {
    /// `nStepsToCenter`, the ring a node is drawn on.
    pub(super) depth: Vec<u32>,
    /// `SPARENT`: the node it was discovered from.
    pub(super) parent: Vec<u32>,
    /// `STSIZE`: how many leaves hang below it, itself included when it is one.
    pub(super) leaves: Vec<u32>,
    /// The search order, a parent always before its children.
    pub(super) order: Vec<u32>,
}

impl Tree {
    /// The column count: one slot per node, since the dense index is the node id here.
    pub(super) fn slots(&self) -> usize {
        self.depth.len()
    }
}

/// `component`'s tree, rooted at `root`.
pub(super) fn grow(neighbours: &Neighbours, component: &[u32], root: u32) -> Tree {
    let (depth, parent, children, order) = search(neighbours, root);
    let leaves = count_leaves(component, &parent, &children);
    Tree {
        depth,
        parent,
        leaves,
        order,
    }
}

/// The FIFO search: depth, parent, child count and the order, in one pass.
fn search(neighbours: &Neighbours, root: u32) -> (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>) {
    let mut depth = vec![NONE; neighbours.rows()];
    let mut parent = vec![NONE; neighbours.rows()];
    let mut children = vec![0_u32; neighbours.rows()];
    let mut order = vec![root];
    depth[root as usize] = 0;
    let mut at = 0;
    while at < order.len() {
        let node = order[at];
        at += 1;
        let next = depth[node as usize] + 1;
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if next < depth[other as usize] {
                found.push(other);
            }
        });
        for other in found {
            // A parallel edge names the same child twice in `found`, and the reference
            // relaxes `SPARENT` the moment it discovers a node — so the second one is
            // already discovered and must not be enqueued, or it is counted twice as a
            // child and drawn in a second slot.
            if parent[other as usize] != NONE {
                continue;
            }
            depth[other as usize] = next;
            parent[other as usize] = node;
            children[node as usize] += 1;
            order.push(other);
        }
    }
    (depth, parent, children, order)
}

/// `STSIZE`: every node without children is one leaf and adds itself and each ancestor,
/// walking up the parents it was given.
///
/// Ascending node order over `component`, as `agfstnode` (`circle.c:174-182`); the counts
/// only ever increment, so the order cannot change the result, and taking it anyway keeps
/// the port's traversal identical to the reference's.
fn count_leaves(component: &[u32], parent: &[u32], children: &[u32]) -> Vec<u32> {
    let mut leaves = vec![0_u32; parent.len()];
    for &node in component {
        if children[node as usize] > 0 {
            continue;
        }
        leaves[node as usize] += 1;
        let mut up = parent[node as usize];
        while up != NONE {
            leaves[up as usize] += 1;
            up = parent[up as usize];
        }
    }
    leaves
}
