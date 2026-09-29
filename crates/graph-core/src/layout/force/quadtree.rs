//! An iterative point quadtree over the dense position arrays, structurally ported from
//! d3-quadtree (`/home/user/refs/npm/d3-quadtree-3.0.1/src/{add,cover,visit,visitAfter}.js`,
//! pinned in `node_modules`): the same bounding-square growth (`cover`), the same
//! per-point insertion (`add`, already iterative — a `while` loop, no recursion), the same
//! exact-coincidence chaining. Traversal ([`Quadtree::visit`], [`Quadtree::postorder_into`])
//! uses an **explicit, reused** stack (devil C11): wasm32's default stack is far smaller
//! than native's, so a naive recursive walk over nearly-coincident points could overflow
//! on wasm only. Every `Vec` here is `clear()`-ed and refilled, never reallocated once its
//! capacity reaches steady state (`dsa-and-memory.md`).
//!
//! Sibling order need not match d3 bit-for-bit — this phase gates on stress quality, not
//! on matching d3's rounding — but `visit.js`'s `3,2,1,0` push order is copied anyway: it
//! costs nothing and keeps the port legible against its reference.

mod bounds;

pub(crate) use bounds::Bounds;

/// One arena slot: an internal node's up to 4 children, or a leaf's chain head.
#[derive(Debug, Clone, Copy)]
enum Shape {
    Internal([Option<u32>; 4]),
    Leaf(u32),
}

/// The point arrays, bundled so build methods stay under the 4-parameter cap.
#[derive(Clone, Copy)]
struct Points<'a> {
    xs: &'a [f64],
    ys: &'a [f64],
}

impl Points<'_> {
    fn at(&self, i: u32) -> (f64, f64) {
        (self.xs[i as usize], self.ys[i as usize])
    }
}

/// The valid (non-NaN) points' extent, `(x0, y0, x1, y1)` (d3's `addAll`, `add.js:61-70`);
/// a NaN point is excluded, then ignored again on insertion (`add.js:8`), matching d3.
fn bounds_of(pts: Points<'_>) -> Option<(f64, f64, f64, f64)> {
    let ok = |(&x, &y): (&f64, &f64)| (!x.is_nan() && !y.is_nan()).then_some((x, y));
    let mut valid = pts.xs.iter().zip(pts.ys).filter_map(ok);
    let (fx, fy) = valid.next()?;
    Some(valid.fold((fx, fy, fx, fy), |(x0, y0, x1, y1), (x, y)| {
        (x0.min(x), y0.min(y), x1.max(x), y1.max(y))
    }))
}

/// A reused Barnes-Hut quadtree: rebuild every tick with [`build`](Quadtree::build), no
/// per-tick allocation once buffers reach steady-state capacity.
#[derive(Debug, Clone, Default)]
pub(crate) struct Quadtree {
    root_bounds: Bounds,
    shape: Vec<Shape>,
    root: Option<u32>,
    chain_next: Vec<Option<u32>>,
    stack_a: Vec<u32>,
    stack_b: Vec<u32>,
    visit_stack: Vec<(u32, Bounds)>,
}

impl Quadtree {
    /// Number of arena nodes (internal or leaf) after the last [`build`](Self::build).
    pub(crate) fn len(&self) -> u32 {
        self.shape.len() as u32
    }

    /// `true` when this node is internal; its up to 4 children, empty slots `None`.
    pub(crate) fn children(&self, node: u32) -> Option<[Option<u32>; 4]> {
        match self.shape[node as usize] {
            Shape::Internal(c) => Some(c),
            Shape::Leaf(_) => None,
        }
    }

    /// Points chained at leaf `node` (most-recent-first; coincident, so order is moot).
    pub(crate) fn leaf_points(&self, node: u32) -> impl Iterator<Item = u32> + '_ {
        let mut cur = match self.shape[node as usize] {
            Shape::Leaf(h) => Some(h),
            Shape::Internal(_) => None,
        };
        core::iter::from_fn(move || {
            let c = cur?;
            cur = self.chain_next[c as usize];
            Some(c)
        })
    }

    /// Rebuilds over `xs`/`ys` (same length), point `i` inserted ascending — fixed,
    /// deterministic order. Clears and refills every buffer.
    pub(crate) fn build(&mut self, xs: &[f64], ys: &[f64]) {
        self.shape.clear();
        self.chain_next.clear();
        self.chain_next.resize(xs.len(), None);
        self.root = None;
        self.root_bounds = Bounds::default();
        let pts = Points { xs, ys };
        let Some((x0, y0, x1, y1)) = bounds_of(pts) else {
            return;
        };
        self.cover(x0, y0);
        self.cover(x1, y1);
        let mut builder = Builder { tree: self, pts };
        for i in 0..xs.len() as u32 {
            builder.add(i);
        }
    }

    /// Grows the root square to cover `(x, y)` (d3's `cover.js`). Only [`build`](Self::build)
    /// calls it, always on an empty tree, so `cover.js`'s non-empty-tree re-root never fires.
    fn cover(&mut self, x: f64, y: f64) {
        if self.root_bounds.x0.is_nan() {
            let x1 = libm::floor(x) + 1.0;
            let y1 = libm::floor(y) + 1.0;
            self.root_bounds = Bounds {
                x0: x1 - 1.0,
                y0: y1 - 1.0,
                x1,
                y1,
            };
            return;
        }
        let b = &mut self.root_bounds;
        let mut z = if b.x1 - b.x0 != 0.0 { b.x1 - b.x0 } else { 1.0 };
        while b.x0 > x || x >= b.x1 || b.y0 > y || y >= b.y1 {
            let i = (((y < b.y0) as usize) << 1) | ((x < b.x0) as usize);
            z *= 2.0;
            match i {
                0 => (b.x1, b.y1) = (b.x0 + z, b.y0 + z),
                1 => (b.x0, b.y1) = (b.x1 - z, b.y0 + z),
                2 => (b.x1, b.y0) = (b.x0 + z, b.y1 - z),
                _ => (b.x0, b.y0) = (b.x1 - z, b.y1 - z),
            }
        }
    }

    fn push(&mut self, shape: Shape) -> u32 {
        self.shape.push(shape);
        (self.shape.len() - 1) as u32
    }

    fn set_child(&mut self, parent: u32, slot: usize, child: u32) {
        if let Shape::Internal(children) = &mut self.shape[parent as usize] {
            children[slot] = Some(child);
        }
    }

    /// A bottom-up node order into `out` (d3's two-stack `visitAfter.js` trick):
    /// descendants of every node precede it. `out` is cleared and refilled, not realloced.
    pub(crate) fn postorder_into(&mut self, out: &mut Vec<u32>) {
        self.stack_a.clear();
        self.stack_b.clear();
        self.stack_a.extend(self.root);
        while let Some(node) = self.stack_a.pop() {
            if let Shape::Internal(children) = self.shape[node as usize] {
                self.stack_a.extend(children.into_iter().flatten());
            }
            self.stack_b.push(node);
        }
        out.clear();
        out.extend(self.stack_b.iter().rev());
    }

    /// A pruned preorder walk (d3's `visit.js`): `prune` runs on every node reached, and a
    /// `true` return skips its children. Children queue `3,2,1,0`, so they visit `0,1,2,3`.
    pub(crate) fn visit(&mut self, prune: impl FnMut(&Self, u32, Bounds) -> bool) {
        // The stack is moved out and back rather than borrowed in place: `visit_in` takes
        // `&self`, and `&mut self.visit_stack` alongside `&self` is two borrows of one
        // struct. Moving keeps the buffer's capacity between calls (the reason it is a
        // field at all) and leaves the field empty only for the duration of the walk.
        let mut stack = core::mem::take(&mut self.visit_stack);
        self.visit_in(&mut stack, prune);
        self.visit_stack = stack;
    }

    /// [`visit`](Self::visit) over a caller-owned stack, so a `&self` walk is possible.
    ///
    /// This is what makes a range kernel over the tree legal (D10): `visit` needs `&mut`
    /// only for its reused stack, so without this the walk would be a `&mut` borrow of
    /// shared start-of-step state and no two workers could take it at once. The order is
    /// `visit`'s own — a pop from a per-caller stack visits the same nodes in the same
    /// sequence as a pop from the tree's — so this is a buffer, not a behaviour.
    pub(crate) fn visit_in(
        &self,
        stack: &mut Vec<(u32, Bounds)>,
        mut prune: impl FnMut(&Self, u32, Bounds) -> bool,
    ) {
        stack.clear();
        if let Some(root) = self.root {
            stack.push((root, self.root_bounds));
        }
        while let Some((node, bounds)) = stack.pop() {
            if prune(self, node, bounds) {
                continue;
            }
            let Some(children) = self.children(node) else {
                continue;
            };
            for slot in (0..4).rev() {
                if let Some(child) = children[slot] {
                    stack.push((child, bounds.quadrant(slot)));
                }
            }
        }
    }
}

/// `&mut Quadtree` plus the points, so `add`/`insert_leaf` stay under the 4-param limit.
struct Builder<'a, 'b> {
    tree: &'a mut Quadtree,
    pts: Points<'b>,
}

impl Builder<'_, '_> {
    /// `add.js:7-48`: descend while internal; insert, chain a coincidence, or split.
    fn add(&mut self, point: u32) {
        let (x, y) = self.pts.at(point);
        if x.is_nan() || y.is_nan() {
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
    /// for the root). Chains an exact coincidence in place; else splits until separated.
    fn insert_leaf(
        &mut self,
        point: u32,
        node: u32,
        parent: Option<(u32, usize)>,
        mut bounds: Bounds,
    ) {
        let (x, y) = self.pts.at(point);
        let head = self
            .tree
            .leaf_points(node)
            .next()
            .expect("leaf holds a point");
        let (xp, yp) = self.pts.at(head);
        if x == xp && y == yp {
            self.tree.chain_next[point as usize] = Some(head);
            self.tree.shape[node as usize] = Shape::Leaf(point);
            return;
        }
        let (mut at, mut slot) = parent.map_or((None, 0), |(p, s)| (Some(p), s));
        loop {
            let (i, j) = bounds.split_step(x, y, xp, yp);
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
}

#[cfg(test)]
mod tests;
