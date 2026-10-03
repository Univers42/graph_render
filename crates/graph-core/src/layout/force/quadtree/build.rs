//! The insertion half of a build, split from `quadtree.rs` for the house 300-line cap.
//! (`hierarchy.rs` + `hierarchy/*.rs` is the precedent this module already follows
//! with `bounds.rs` and `preorder.rs`.)

use std::cmp::Ordering;

use super::{Bounds, Points, Quadtree, Shape};

/// `&mut Quadtree` plus the points, so `add`/`insert_leaf` take at most four parameters
/// besides the receiver (`bounds::split_step` is at the same count).
///
/// Ponytail: `insert_leaf`'s bail chains two points the quadtree can no longer tell apart —
/// after the bounds stop narrowing, which is one `split_step` past the last place an `f64`
/// midpoint can fall between them — so it costs that pair its own exact charge term and
/// nothing else. It gets the geometry of a chain, one `1e-6` jiggle apart, for two
/// coordinates that differ by less than a rounding step of the square holding them.
/// Escape hatch: give the points coordinates the `f64` grid can separate, or raise the
/// `distanceMin` the chain then divides by.
pub(super) struct Builder<'a, 'b> {
    pub(super) tree: &'a mut Quadtree,
    pub(super) pts: Points<'b>,
}

impl Builder<'_, '_> {
    /// `add.js:7-48`: descend while internal; insert, chain a coincidence, or split.
    pub(super) fn add(&mut self, point: u32) {
        let (x, y) = self.pts.at(point);
        if !x.is_finite() || !y.is_finite() {
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
            let slot = bounds.narrow(x, y);
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

    /// `add.js:36-47`: `node` is a leaf at `bounds`, reached via `parent`'s slot (`None`
    /// for the root). Chains an exact coincidence in place; else splits until separated,
    /// or until the bounds stop narrowing, which is the same thing here.
    fn insert_leaf(
        &mut self,
        point: u32,
        node: u32,
        parent: Option<(u32, usize)>,
        mut bounds: Bounds,
    ) {
        let (x, y) = self.pts.at(point);
        let head = self.tree.head(node).expect("leaf holds a point");
        let (xp, yp) = self.pts.at(head);
        if x == xp && y == yp {
            self.chain(point, node);
            return;
        }
        let (mut at, mut slot) = parent.map_or((None, 0), |(p, s)| (Some(p), s));
        loop {
            let before = bounds.span();
            let (i, j) = bounds.split_step(x, y, xp, yp);
            // `NaN` is `None` here, so this one test is also the bail for a square that
            // overflowed — written as `partial_cmp` because `>=` would say nothing about
            // the incomparable case.
            if bounds.span().partial_cmp(&before) != Some(Ordering::Less) {
                self.chain(point, node);
                return;
            }
            let internal = self.tree.push(Shape::Internal([None; 4]));
            match at {
                Some(p) => self.tree.set_child(p, slot, internal),
                None => self.tree.root = Some(internal),
            }
            if i != j {
                self.tree.set_child(internal, j, node);
                let leaf = self.tree.push(Shape::Leaf(point));
                self.tree.set_child(internal, i, leaf);
                return;
            }
            (at, slot) = (Some(internal), i);
        }
    }

    /// `point` becomes `node`'s new chain head — an exact coincidence, or two points the
    /// tree cannot separate (`insert_leaf`'s bail above). Either way every point stays in
    /// exactly one leaf's chain, so `order` is still a permutation of the points.
    fn chain(&mut self, point: u32, node: u32) {
        self.tree.chain_next[point as usize] = self.tree.head(node);
        self.tree.shape[node as usize] = Shape::Leaf(point);
    }
}
