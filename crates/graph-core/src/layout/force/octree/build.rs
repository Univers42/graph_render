//! The insertion half of a build, split from `octree.rs` for the house 300-line cap (the
//! split `quadtree.rs`/`build.rs` already uses). Every branch is the quadtree's own with a
//! third axis in the slot; nothing here is shared with it, which is the point — see
//! `octree.rs`'s header.

use std::cmp::Ordering;

use super::{Bounds3, Octree, Points3, Shape, OCTANTS};

/// `&mut Octree` plus the points, so `add`/`insert_leaf` take at most four parameters
/// besides the receiver.
///
/// Ponytail: `insert_leaf`'s bail chains two points the octree can no longer tell apart —
/// after the bounds stop narrowing, which is one `split_step` past the last place an `f64`
/// midpoint can fall between them — so it costs that pair its own exact charge term and
/// nothing else. It gets the geometry of a chain, one jiggle apart, for two coordinates
/// that differ by less than a rounding step of the cube holding them. Escape hatch: give
/// the points coordinates the `f64` grid can separate.
pub(super) struct Builder<'a, 'b> {
    pub(super) tree: &'a mut Octree,
    pub(super) pts: Points3<'b>,
}

impl Builder<'_, '_> {
    /// Descend while internal; insert, chain a coincidence, or split.
    pub(super) fn add(&mut self, point: u32) {
        let p = self.pts.at(point);
        if !p.0.is_finite() || !p.1.is_finite() || !p.2.is_finite() {
            return;
        }
        let Some(mut node) = self.tree.root else {
            self.tree.root = Some(self.tree.push(Shape::Leaf(point)));
            return;
        };
        let mut bounds = self.tree.root_bounds;
        let mut parent = None;
        loop {
            let Some(children) = self.tree.children(node) else {
                self.insert_leaf(point, node, parent, bounds);
                return;
            };
            let slot = bounds.narrow(p.0, p.1, p.2);
            match children[slot] {
                Some(child) => {
                    parent = Some((node, slot));
                    node = child;
                }
                None => {
                    let leaf = self.tree.push(Shape::Leaf(point));
                    self.tree.set_child(node, slot, leaf);
                    return;
                }
            }
        }
    }

    /// `node` is a leaf at `bounds`, reached via `parent`'s slot (`None` for the root).
    /// Chains an exact coincidence in place; else splits until separated, or until the
    /// bounds stop narrowing, which is the same thing here.
    fn insert_leaf(
        &mut self,
        point: u32,
        node: u32,
        parent: Option<(u32, usize)>,
        mut bounds: Bounds3,
    ) {
        let p = self.pts.at(point);
        let head = self.tree.head(node).expect("leaf holds a point");
        let q = self.pts.at(head);
        if p == q {
            self.chain(point, node);
            return;
        }
        let (mut at, mut slot) = parent.map_or((None, 0), |(p, s)| (Some(p), s));
        loop {
            let before = bounds.span();
            let (i, j) = bounds.split_step(p, q);
            // `NaN` is `None` here, so this one test is also the bail for a cube that
            // overflowed — written as `partial_cmp` because `>=` would say nothing about
            // the incomparable case.
            if bounds.span().partial_cmp(&before) != Some(Ordering::Less) {
                self.attach(at, slot, node);
                self.chain(point, node);
                return;
            }
            let internal = self.tree.push(Shape::Internal(empty_children()));
            self.attach(at, slot, internal);
            if i != j {
                self.tree.set_child(internal, j, node);
                let leaf = self.tree.push(Shape::Leaf(point));
                self.tree.set_child(internal, i, leaf);
                return;
            }
            (at, slot) = (Some(internal), i);
        }
    }

    /// Hangs `child` at `parent`'s `slot`, or makes it the root. A bail re-hangs the leaf
    /// it split from at the deepest internal node pushed so far: without that, a bail
    /// after the first split left both points in no leaf (`order` lost them).
    fn attach(&mut self, parent: Option<u32>, slot: usize, child: u32) {
        match parent {
            Some(p) => self.tree.set_child(p, slot, child),
            None => self.tree.root = Some(child),
        }
    }

    /// `point` becomes `node`'s new chain head — an exact coincidence, or two points the
    /// tree cannot separate. Either way every point stays in exactly one leaf's chain, so
    /// `order` is still a permutation of the points.
    fn chain(&mut self, point: u32, node: u32) {
        self.tree.chain_next[point as usize] = self.tree.head(node);
        self.tree.shape[node as usize] = Shape::Leaf(point);
    }
}

/// The empty child array an internal node is pushed with; named so the slot count lives in
/// one place rather than as a bare `[None; OCTANTS]`.
pub(super) fn empty_children() -> [Option<u32>; OCTANTS] {
    [None; OCTANTS]
}