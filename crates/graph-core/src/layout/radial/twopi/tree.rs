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
//!
//! Every pass writes into a [`Scratch`] the caller reset over this component, rather than
//! into columns of its own: see L-01 in that module.

use super::adjacency::Neighbours;
use super::scratch::Scratch;

/// No parent: the root, and every node outside this component.
pub(super) const NONE: u32 = u32::MAX;

/// `component`'s tree, rooted at `root`, into `scratch`.
pub(super) fn grow(neighbours: &Neighbours, component: &[u32], root: u32, scratch: &mut Scratch) {
    search(neighbours, root, scratch);
    count_leaves(component, scratch);
}

/// The FIFO search: depth, parent, child count and the order, in one pass over
/// `scratch.order`, which the caller has emptied.
fn search(neighbours: &Neighbours, root: u32, scratch: &mut Scratch) {
    scratch.depth[root as usize] = 0;
    scratch.order.push(root);
    let mut at = 0;
    while at < scratch.order.len() {
        let node = scratch.order[at];
        at += 1;
        let next = scratch.depth[node as usize] + 1;
        let mut found = Vec::new();
        neighbours.for_each(node, |other| {
            if next < scratch.depth[other as usize] {
                found.push(other);
            }
        });
        for other in found {
            // A parallel edge names the same child twice in `found`, and the reference
            // relaxes `SPARENT` the moment it discovers a node — so the second one is
            // already discovered and must not be enqueued, or it is counted twice as a
            // child and drawn in a second slot.
            if scratch.parent[other as usize] != NONE {
                continue;
            }
            scratch.depth[other as usize] = next;
            scratch.parent[other as usize] = node;
            scratch.children[node as usize] += 1;
            scratch.order.push(other);
        }
    }
}

/// `STSIZE`: every node without children is one leaf and adds itself and each ancestor,
/// walking up the parents it was given.
///
/// Ascending node order over `component`, as `agfstnode` (`circle.c:174-182`); the counts
/// only ever increment, so the order cannot change the result, and taking it anyway keeps
/// the port's traversal identical to the reference's.
fn count_leaves(component: &[u32], scratch: &mut Scratch) {
    for &node in component {
        if scratch.children[node as usize] > 0 {
            continue;
        }
        scratch.leaves[node as usize] += 1;
        let mut up = scratch.parent[node as usize];
        while up != NONE {
            scratch.leaves[up as usize] += 1;
            up = scratch.parent[up as usize];
        }
    }
}
