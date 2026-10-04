//! ForceAtlas2's repulsion over a quadtree: O(n log n) per iteration instead of the dense
//! O(n²) pair loop, the approximation Gephi's own ForceAtlas2 offers (`barnesHutOptimize`),
//! at a tighter opening angle than Gephi's θ = 1.2.
//!
//! Each iteration rebuilds the shared Barnes-Hut [`Quadtree`] over the current positions,
//! sums every cell's mass and centre of mass bottom-up, then gathers each node's force in
//! one stackless preorder walk: a cell far enough away (`w² < θ² d²`, `w` its side, `d` the
//! distance to its centre of mass) acts as one body of its total mass; a leaf that is not
//! acts pair by pair, with the dense loop's own `repel_delta` and force expression. A node
//! writes only its own `ux`/`uy`, so the order nodes are visited in moves no float.
//!
//! Caveat: a far cell is a point mass at its centre, so a node's repulsion is off by the
//! cell's spread; failing input is a settled layout, where a node's repulsion nearly cancels
//! and the far field is most of what is left. Direction: at θ = 0.8, after 100 iterations,
//! up to 1.1% of a node's force and 4.0% of the whole field (1500 and 5000 nodes); Gephi's
//! 1.2 reached 7.1% and 18.7% (`docs/measurements/perf-fa2bh.md`), so it is not used.
//! Escape hatch: `layout.forceatlas2`, the exact dense sum, kept as this layout's oracle.

use super::Fa2State;
use crate::layout::force::quadtree::{Cell, Quadtree};

/// The opening angle, squared: θ = 0.8, the loosest angle whose error stayed near 1% per
/// node on a settled layout in the sweep `docs/measurements/perf-fa2bh.md` records.
pub(super) const THETA2: f64 = 0.8 * 0.8;

/// One cell's total mass and its centre of mass.
#[derive(Debug, Clone, Copy, Default)]
struct Body {
    mass: f64,
    cx: f64,
    cy: f64,
}

/// The tree and its per-cell bodies, both rebuilt in place every iteration.
#[derive(Debug, Default)]
pub(super) struct Tree {
    quadtree: Quadtree,
    bodies: Vec<Body>,
    /// The x and y columns gathered out of the row-major positions once per iteration.
    /// The quadtree wants one contiguous slice per axis (`Quadtree::build`), and the state
    /// now carries `(n, dim)` rows, so the columns are materialised here rather than by a
    /// second per-node field on the state. Both are rebuilt every iteration alongside
    /// `quadtree`, so nothing is carried across an iteration.
    xs: Vec<f64>,
    ys: Vec<f64>,
}

impl Tree {
    /// Gathers the two live axes of `state`'s rows into the contiguous columns the
    /// quadtree builds over. Axis order is ascending, so `xs` then `ys` is the order the
    /// dense 2D arm always drew its two coordinates in.
    fn gather(&mut self, state: &Fa2State) {
        self.xs.clear();
        self.ys.clear();
        for row in &state.p {
            self.xs.push(row[0]);
            self.ys.push(row[1]);
        }
    }
}

impl Tree {
    /// Sums the bodies leaves first (reverse preorder, so every child is done before its
    /// parent), then turns each sum into a centre.
    fn aggregate(&mut self, state: &Fa2State) {
        let (cells, order) = (self.quadtree.cells(), self.quadtree.order());
        self.bodies.clear();
        self.bodies.resize(cells.len(), Body::default());
        for k in (0..cells.len()).rev() {
            let sum = if cells[k].skip as usize == k + 1 {
                leaf_sum(
                    state,
                    &order[cells[k].start as usize..cells[k].end as usize],
                )
            } else {
                children_sum(cells, &self.bodies, k)
            };
            self.bodies[k] = sum;
        }
        for body in &mut self.bodies {
            (body.cx, body.cy) = (body.cx / body.mass, body.cy / body.mass);
        }
    }
}

/// A leaf's mass and its mass-weighted coordinate sums.
fn leaf_sum(state: &Fa2State, points: &[u32]) -> Body {
    let mut sum = Body::default();
    for &p in points {
        let (m, p) = (state.mass[p as usize], p as usize);
        sum.mass += m;
        sum.cx += m * state.p[p][0];
        sum.cy += m * state.p[p][1];
    }
    sum
}

/// An internal cell's sums: its children's, in slot order (child `c`'s sibling is
/// `cells[c].skip`).
fn children_sum(cells: &[Cell], bodies: &[Body], k: usize) -> Body {
    let mut sum = Body::default();
    let mut c = k + 1;
    while c < cells[k].skip as usize {
        sum.mass += bodies[c].mass;
        sum.cx += bodies[c].cx;
        sum.cy += bodies[c].cy;
        c = cells[c].skip as usize;
    }
    sum
}

impl Fa2State {
    /// The repulsion pass over `tree`, at opening angle² `theta2`: the dense
    /// [`repulsion`](Fa2State::repulsion)'s forces, approximated far away.
    pub(super) fn repel_tree(&mut self, tree: &mut Tree, theta2: f64) {
        tree.gather(self);
        tree.quadtree.build(&tree.xs, &tree.ys);
        tree.aggregate(self);
        let order = tree.quadtree.order();
        for (p, &i) in order.iter().enumerate() {
            let (fx, fy) = self.walk(tree, p as u32, theta2);
            self.u[i as usize][0] += fx;
            self.u[i as usize][1] += fy;
        }
    }

    /// The force on the node at tree position `p` (`order[p]`), one preorder walk.
    fn walk(&self, tree: &Tree, p: u32, theta2: f64) -> (f64, f64) {
        let (cells, order) = (tree.quadtree.cells(), tree.quadtree.order());
        let i = order[p as usize];
        let (xi, yi, mi) = (
            self.p[i as usize][0],
            self.p[i as usize][1],
            self.mass[i as usize],
        );
        let k_ratio = self.params.scaling_ratio;
        let (mut fx, mut fy, mut k) = (0.0, 0.0, 0);
        while k < cells.len() {
            let cell = cells[k];
            let holds = cell.start <= p && p < cell.end;
            let body = tree.bodies[k];
            let (dx, dy) = (xi - body.cx, yi - body.cy);
            let d2 = dx * dx + dy * dy;
            let w = cell.bounds.x1 - cell.bounds.x0;
            if !holds && d2 > 0.0 && w * w < theta2 * d2 {
                let f = mi * body.mass / d2 * k_ratio;
                (fx, fy) = (fx + dx * f, fy + dy * f);
                k = cell.skip as usize;
            } else if cell.skip as usize == k + 1 {
                let leaf = &order[cell.start as usize..cell.end as usize];
                let (lx, ly) = self.pairs(i, leaf);
                (fx, fy) = (fx + lx, fy + ly);
                k += 1;
            } else {
                k += 1;
            }
        }
        (fx, fy)
    }

    /// The exact force on `i` from every other point of one leaf: the dense loop's pair,
    /// from `i`'s side (`repel_delta` is antisymmetric, the coincidence jiggle included).
    fn pairs(&self, i: u32, leaf: &[u32]) -> (f64, f64) {
        let k_ratio = self.params.scaling_ratio;
        let (mut fx, mut fy) = (0.0, 0.0);
        for &j in leaf.iter().filter(|&&j| j != i) {
            // `repel_delta` is antisymmetric, the coincidence jiggle included, so the
            // pair is taken from the lower index and negated when `i` is the higher one.
            // Only the two in-plane axes are read: the tree arm pins `dim` at 2, so this
            // is an axis projection of the row, never a truncation of it.
            let d = if i < j {
                self.repel_delta(i, j)
            } else {
                let d = self.repel_delta(j, i);
                [-d[0], -d[1], d[2]]
            };
            let (dx, dy) = (d[0], d[1]);
            let d2 = dx * dx + dy * dy;
            let f = self.mass[i as usize] * self.mass[j as usize] / d2 * k_ratio;
            (fx, fy) = (fx + dx * f, fy + dy * f);
        }
        (fx, fy)
    }
}

#[cfg(test)]
mod tests;
