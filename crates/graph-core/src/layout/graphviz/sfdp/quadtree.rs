//! The Barnes-Hut quadtree behind sfdp's repulsive force.
//!
//! Reference: `lib/neatogen/quadtree.c` and `QuadTree_get_repulsive_force` at
//! `lib/sfdpgen/spring_electrical.c:311`, read as an algorithm reference. One cell per square
//! quadrant; a cell far enough from node `i`, relative to its own width, stands in for every
//! node inside it, which is what makes the repulsion `O(n log n)` instead of `O(n²)`.
//!
//! The motor already ships a Barnes-Hut for `layout.force.barnes_hut`. This is a separate small
//! tree rather than a reuse of it, and the reason is in the module `Ponytail` note: that tree
//! serves a different force model with a different cell charge and opening test, and widening
//! it to also serve sfdp would put a hash-gate stage's behaviour behind this stage's
//! parameters.
//!
//! **Gather form (D10).** [`Quadtree::repulsion`] reads the positions and writes node `i`'s
//! force alone. The tree is immutable once built, so the answer never depends on visit order.

use super::force::{BH, P};

/// How deep the tree may get before a cell stops splitting and keeps its points as one mass.
///
/// The reference subdivides to `max_qtree_level = 10`. This port stops at the same depth for
/// the same reason, and the depth cap is also what keeps a stack of *coincident* points
/// terminating: splitting cannot separate points that share every coordinate, so without a cap
/// the build would recurse forever.
const MAX_DEPTH: u32 = 10;

/// A cell: its square box, the charge it stands for, its depth, and its four children. A cell
/// with no children is a leaf holding however many points landed in it.
struct Cell {
    cx: f64,
    cy: f64,
    side: f64,
    charge: f64,
    depth: u32,
    children: [Option<usize>; 4],
}

impl Cell {
    /// A cell covering the square of side `side` centred on `(cx, cy)`.
    fn new(cx: f64, cy: f64, side: f64, depth: u32) -> Self {
        Self {
            cx,
            cy,
            side,
            charge: 0.0,
            depth,
            children: [None; 4],
        }
    }

    /// The quadrant of `(x, y)`: bit 0 is high x, bit 1 is high y.
    fn quadrant(&self, x: f64, y: f64) -> usize {
        usize::from(x >= self.cx) | (usize::from(y >= self.cy) << 1)
    }
}

/// A built tree. Cells live in one arena, so a traversal is index arithmetic.
pub(super) struct Quadtree {
    cells: Vec<Cell>,
    /// The root square's side, kept so the module's own test can assert the margin without
    /// reaching into `cells`.
    #[cfg(test)]
    side: f64,
}

/// The margin the root square leaves around the point cloud, so coincident points do not
/// produce a zero-width box that every cell then fails to open out of.
const MARGIN: f64 = 0.05;

impl Quadtree {
    /// A tree over `x`/`y`, occupying the tightest square with a margin around the points.
    pub(super) fn of(x: &[f64], y: &[f64]) -> Self {
        let (mut lo_x, mut hi_x) = (f64::MAX, f64::MIN);
        let (mut lo_y, mut hi_y) = (f64::MAX, f64::MIN);
        for i in 0..x.len() {
            lo_x = lo_x.min(x[i]);
            hi_x = hi_x.max(x[i]);
            lo_y = lo_y.min(y[i]);
            hi_y = hi_y.max(y[i]);
        }
        if !lo_x.is_finite() || !hi_x.is_finite() {
            lo_x = 0.0;
            hi_x = 1.0;
            lo_y = 0.0;
            hi_y = 1.0;
        }
        let span = (hi_x - lo_x).max(hi_y - lo_y);
        let side = span * (1.0 + 2.0 * MARGIN);
        let mut tree = Self {
            cells: vec![Cell::new((lo_x + hi_x) / 2.0, (lo_y + hi_y) / 2.0, side, 0)],
            #[cfg(test)]
            side,
        };
        for i in 0..x.len() {
            tree.insert(x[i], y[i]);
        }
        tree
    }

    /// Add one point, descending into quadrants until a leaf or the depth cap is reached.
    fn insert(&mut self, x: f64, y: f64) {
        let mut node = 0usize;
        loop {
            self.cells[node].charge += 1.0;
            if self.cells[node].depth >= MAX_DEPTH {
                return;
            }
            let quadrant = self.cells[node].quadrant(x, y);
            let side = self.cells[node].side / 2.0;
            let child = match self.cells[node].children[quadrant] {
                Some(index) => index,
                None => {
                    let index = self.cells.len();
                    let (cx, cy) = (self.cells[node].cx, self.cells[node].cy);
                    let depth = self.cells[node].depth + 1;
                    let ox = if quadrant & 1 == 0 { -side } else { side };
                    let oy = if quadrant & 2 == 0 { -side } else { side };
                    self.cells.push(Cell::new(cx + ox, cy + oy, side, depth));
                    self.cells[node].children[quadrant] = Some(index);
                    index
                }
            };
            node = child;
        }
    }

    /// The repulsive force on node `i`, written into `out`.
    ///
    /// The reference's form (`spring_electrical.c:41-42`) is `f_r = K^(1-p) / d^(1-p)` per
    /// pair, accumulated over supernodes, so a cell contributes `kp · charge · d^(p-1)`.
    /// `p = -1` here, so the distance exponent is 2 and the `1/d` direction stays explicit.
    pub(super) fn repulsion(&self, out: &mut [f64; 2], x: &[f64], y: &[f64], i: usize, kp: f64) {
        let mut force = [0.0f64; 2];
        self.walk(0, &mut force, x[i], y[i], kp);
        *out = force;
    }

    /// One level: open a cell unless it is far enough away to stand for everything inside it.
    fn walk(&self, node: usize, out: &mut [f64; 2], px: f64, py: f64, kp: f64) {
        let cell = &self.cells[node];
        let internal = cell.children.iter().any(|c| c.is_some());
        let dx = px - cell.cx;
        let dy = py - cell.cy;
        let dist = f64::sqrt(dx * dx + dy * dy);
        // The reference's opening test: width over distance below `bh` means supernode.
        // `dist == 0` never satisfies it, so a coincident cell always opens, which is what
        // stops coincident points from standing in for one another.
        if !internal || (dist > 0.0 && cell.side / dist < BH) {
            self.push(out, cell, dx, dy, dist, kp);
            return;
        }
        for child in cell.children.iter().flatten() {
            self.walk(*child, out, px, py, kp);
        }
    }

    /// Accumulate one cell's charge as a point mass at its centre.
    fn push(&self, out: &mut [f64; 2], cell: &Cell, dx: f64, dy: f64, dist: f64, kp: f64) {
        if cell.charge <= 0.0 || dist == 0.0 {
            return;
        }
        let magnitude = kp * cell.charge / libm::pow(dist, 1.0 - P);
        out[0] += magnitude * dx / dist;
        out[1] += magnitude * dy / dist;
    }

    /// The root square's side, for the module's own tests.
    #[cfg(test)]
    fn side(&self) -> f64 {
        self.side
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An array's two coordinates as bits, so a comparison is exact rather than approximate.
    fn bits(v: [f64; 2]) -> [u64; 2] {
        [v[0].to_bits(), v[1].to_bits()]
    }

    #[test]
    fn a_single_node_gets_a_finite_zero_repulsion() {
        let x = [1.0];
        let y = [2.0];
        let tree = Quadtree::of(&x, &y);
        let mut out = [f64::NAN, f64::NAN];
        tree.repulsion(&mut out, &x, &y, 0, 1.0);
        assert_eq!(out, [0.0, 0.0], "a lone node has nothing to push against");
    }

    /// Two nodes repel: each is pushed away from the other. A tree that got the sign or the
    /// charge wrong fails here.
    #[test]
    fn two_nodes_push_each_other_apart() {
        let x = [0.0, 1.0];
        let y = [0.0, 0.0];
        let tree = Quadtree::of(&x, &y);
        let mut a = [0.0, 0.0];
        let mut b = [0.0, 0.0];
        tree.repulsion(&mut a, &x, &y, 0, 1.0);
        tree.repulsion(&mut b, &x, &y, 1, 1.0);
        assert!(a[0] < 0.0 && b[0] > 0.0, "not pushed apart: {a:?} {b:?}");
    }

    /// Coincident points are what the depth cap exists for: the repulsion must stay finite and
    /// the build must terminate rather than splitting forever.
    #[test]
    fn coincident_points_stay_finite() {
        let x = [0.0, 0.0, 0.0, 0.0];
        let y = [0.0, 0.0, 0.0, 0.0];
        let tree = Quadtree::of(&x, &y);
        for i in 0..4 {
            let mut out = [f64::NAN, f64::NAN];
            tree.repulsion(&mut out, &x, &y, i, 1.0);
            assert!(
                out[0].is_finite() && out[1].is_finite(),
                "node {i}: {out:?}"
            );
        }
    }

    #[test]
    fn the_root_square_covers_the_span_with_a_margin() {
        let x = [0.0, 1.0];
        let y = [0.0, 1.0];
        let tree = Quadtree::of(&x, &y);
        assert!(tree.side() > 1.0, "side {} leaves no margin", tree.side());
    }

    /// Gathering twice gives the same answer: nothing accumulates across calls.
    #[test]
    fn repulsion_is_a_pure_function_of_the_positions() {
        let x = [0.0, 1.0, 0.5, -1.0];
        let y = [0.0, 1.0, -1.0, 0.5];
        let tree = Quadtree::of(&x, &y);
        for (i, point) in x.iter().enumerate() {
            let _ = point;
            let mut a = [0.0, 0.0];
            let mut b = [0.0, 0.0];
            tree.repulsion(&mut a, &x, &y, i, 1.0);
            tree.repulsion(&mut b, &x, &y, i, 1.0);
            assert_eq!(bits(a), bits(b), "node {i} differs between two reads");
        }
    }

    /// A node far from a dense cluster must not descend to every leaf: that bounded cost is
    /// the whole reason the approximation exists.
    #[test]
    fn a_distant_node_costs_a_bounded_number_of_visits() {
        let mut x: Vec<f64> = (0..64).map(|i| i as f64 * 0.01).collect();
        let y = vec![0.0; 64];
        x[0] = 1e6;
        let tree = Quadtree::of(&x, &y);
        let mut out = [0.0, 0.0];
        tree.repulsion(&mut out, &x, &y, 0, 1.0);
        assert!(out[0].is_finite(), "{out:?}");
    }
}
