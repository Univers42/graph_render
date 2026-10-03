//! The per-node columns every component's passes write, and the reset that hands a
//! component a clean set of them.
//!
//! Finding L-01: these six columns used to be allocated *per component*. On an edgeless
//! graph — one component per node — that made the run spend `O(components x n)` where
//! `registry/radial.rs` declares `O(n + m)`, and it was the only shape on which the
//! declaration failed. They are allocated once here, for the whole layout.
//!
//! **Resetting only the component's own entries is enough, and it is not a shortcut.**
//! Every write a pass makes is to a node it reached from this component's root, and every
//! read is to a neighbour of a node it is walking — and a neighbour of a component node is
//! in the same component, because `adjacency::for_each` yields a node's out- and in-edges
//! alike. So no entry outside `component` is ever observed, and clearing `n` slots per
//! component would only be the quadratic again wearing a different hat.

use super::UNSET;
use super::tree::NONE;

/// Every per-node column the per-component passes write, one slot per node.
pub(super) struct Scratch {
    /// `nStepsToCenter`, the ring a node is drawn on.
    pub(super) depth: Vec<u32>,
    /// `SPARENT`: the node it was discovered from.
    pub(super) parent: Vec<u32>,
    /// `NCHILD`: how many children the search gave it.
    pub(super) children: Vec<u32>,
    /// `STSIZE`: how many leaves hang below it, itself included when it is one.
    pub(super) leaves: Vec<u32>,
    /// This component's search order, a parent always before its children.
    pub(super) order: Vec<u32>,
    /// `SPAN`, the slice of the circle this node's own subtree is given.
    pub(super) span: Vec<f64>,
    /// `THETA`, the middle of that slice.
    pub(super) theta: Vec<f64>,
}

impl Scratch {
    /// One column per node, allocated once for the whole layout.
    ///
    /// `order` starts at full capacity rather than growing: it ends up holding a whole
    /// component anyway, and its doubling was a dozen allocations inside the component
    /// loop for nothing.
    pub(super) fn new(count: u32) -> Self {
        let slots = count as usize;
        Self {
            depth: vec![NONE; slots],
            parent: vec![NONE; slots],
            children: vec![0; slots],
            leaves: vec![0; slots],
            order: Vec::with_capacity(slots),
            span: vec![0.0; slots],
            theta: vec![UNSET; slots],
        }
    }

    /// Puts `component`'s entries back to "not set", so the next component starts from the
    /// state the first one did. The columns keep their capacity; only the touched slots
    /// are written.
    pub(super) fn reset(&mut self, component: &[u32]) {
        self.order.clear();
        for &node in component {
            let slot = node as usize;
            self.depth[slot] = NONE;
            self.parent[slot] = NONE;
            self.children[slot] = 0;
            self.leaves[slot] = 0;
            self.span[slot] = 0.0;
            self.theta[slot] = UNSET;
        }
    }
}
