//! The block-cutpoint tree: `createBlocktree` (`blocktree.c`), reimplemented.
//!
//! The reference's `dfs` is one recursive walk doing three jobs at once, and this is that
//! walk as an explicit frame stack:
//!
//! 1. `VAL`/`LOWVAL` — the classic lowlink pair, with the twist that `VAL(root) == 0`, so
//!    the root reads as an *artificial* cut point (`blocktree.c:72`). Here the counter
//!    starts at `0` and is bumped before each assignment, so every real node gets
//!    `VAL >= 1` and `0` stays "unvisited" — the same encoding, one step later.
//! 2. Popping **blocks**. Every edge pushed on the way down is popped again when its tail's
//!    articulation test fires (`LOWVAL(child) >= VAL(parent)`), and the *head* of each popped
//!    edge joins the block (`blocktree.c:81-92`). The pop runs until the edge it came in on,
//!    which is **not** always the top: an edge whose child closed a cycle back is never
//!    popped at its own tail, so a later pop takes several at once. That is how a triangle
//!    becomes one block.
//! 3. Tree assembly — after the walk, each block but the first hangs on the block holding
//!    its earliest-discovered node (`blocktree.c:155-176`), which is also the node its
//!    `PARENT_F` flag lands on.
//!
//! **`EDGEORDER` is read but never acted on, and that is a measured simplification.** The
//! reference sets `EDGEORDER(e) = +1` when it first meets an edge from its tail and `-1`
//! from its head (`blocktree.c:62-70`), then pops with `np = EDGEORDER == 1 ? head : tail`.
//! Either way `np` is the endpoint *other than the node that pushed the edge* — the
//! discovered child, which is this port's `head`. So the edge stack holds node ids.
//!
//! The walk itself is [`walk`], split out by the house's 300-line limit; this module is the
//! per-component driver around it.

use super::Block;
use super::graph::Derived;

mod walk;

use walk::walk_component;

/// Every connected component's blocks, their roots, and the `PARENT_F` flags.
pub(super) struct Found {
    pub(super) blocks: Vec<Block>,
    /// One root block per component that has more than one node. A single-node component is
    /// the reference's own short circuit (`circular.c:70-74`): its node goes to the origin
    /// and never becomes a block at all.
    pub(super) roots: Vec<usize>,
    /// `PARENT_F` per node: true for the node in a parent block that a child hangs off.
    pub(super) parent_flag: Vec<bool>,
}

/// Every connected component's block tree.
///
/// The reference builds the component list with `ccomps` and lays each one out in turn with
/// a **fresh** `circ_state`, so its `orderCount` restarts at 1 per component
/// (`circular.c:77-83`). [`walk_component`] restarts it the same way, which is what makes
/// the `VAL` numbers comparable inside a component and meaningless across two.
pub(super) fn decompose(derived: &Derived, count: u32) -> Found {
    let mut seen = vec![false; count as usize];
    let mut found = Found {
        blocks: Vec::new(),
        roots: Vec::new(),
        parent_flag: vec![false; count as usize],
    };
    for start in 0..count {
        if seen[start as usize] {
            continue;
        }
        let component = flood(derived, start, &mut seen);
        if component.len() > 1 {
            let root = walk_component(derived, component, &mut found);
            found.roots.push(root);
        }
    }
    found
}

/// The component of `start`, so the loop above starts it exactly once.
fn flood(derived: &Derived, start: u32, seen: &mut [bool]) -> Vec<u32> {
    let mut queue = vec![start];
    seen[start as usize] = true;
    let mut at = 0;
    while at < queue.len() {
        let node = queue[at];
        at += 1;
        for other in derived.neighbours(node) {
            if !seen[other as usize] {
                seen[other as usize] = true;
                queue.push(other);
            }
        }
    }
    queue
}

/// How wide the walk's `slot` array must be: one slot per node of the derived graph, which is
/// the highest index any of this component's nodes can have.
fn node_count(component: &[u32]) -> usize {
    component
        .iter()
        .map(|node| *node as usize + 1)
        .max()
        .unwrap_or(0)
}
