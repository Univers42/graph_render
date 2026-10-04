//! The Barnes-Hut quadtree behind sfdp's repulsive force.
//!
//! Reference: `lib/sparse/QuadTree.c` (`QuadTree_new_from_point_list`, `QuadTree_add_internal`,
//! `QuadTree_get_supernodes_internal`) and its caller at `lib/sfdpgen/spring_electrical.c:586`,
//! read as an algorithm reference. A cell holds one point until a second arrives, then splits
//! and pushes both down; at the depth cap it keeps a list instead. Node `i` sees every point in
//! the leaves it opens exactly (itself excluded), and an internal cell far enough away as one
//! supernode of its whole weight at its **centre of mass**.
//!
//! The motor already ships a Barnes-Hut for `layout.force.barnes_hut`. This is a separate small
//! tree because that one serves d3's force model, with a different charge and opening test.
//!
//! **Gather form (D10).** [`Quadtree::repulsion`] reads the tree and the positions and returns
//! node `i`'s force alone. The tree is immutable once built. This is the *shape* of the
//! reference's `QuadTree_get_supernodes`; the arithmetic is the reference's, and the decision to
//! compute it a whole iteration at a time is `docs/decisions/sfdp-gather-form.md`.
//!
//! **Below 45 nodes this tree does not run at all.** `quadtree_size = 45`
//! (`spring_electrical.c:39`) and `n >= quadtree_size` (`:543`) is the reference's own test, so
//! [`super::solve::Solve::gather`] sums all pairs exactly on a small level. The previous port
//! always walked the tree, which put a cell's **centre** where the reference puts a cell's
//! **centre of mass** — on a 4-node level that is a different force entirely.

use super::force::{self, BH};

/// The reference's `max_qtree_level` (`spring_electrical.c:62`). It also keeps coincident points
/// terminating: splitting cannot separate them, so at this depth a cell keeps a list instead.
///
/// Ponytail: the reference retunes this depth every iteration with `oned_optimizer`
/// (`spring_electrical.c:579`, `:627`) to minimise the walk's cost; here it stays at 10.
/// Failing input: none in the drawing (the depth decides which cells are opened, so the forces
/// differ in the last digits); the cost is a deeper tree than the optimiser would pick on
/// dense clusters. Escape hatch: the constant.
const MAX_DEPTH: u32 = 10;

/// No point, no child.
const NONE: u32 = u32::MAX;

/// One square cell: its centre and half-width, the points it stands for, and either a list of
/// points (`head`, linked through [`Quadtree::next`]) or up to four children.
struct Cell {
    cx: f64,
    cy: f64,
    half: f64,
    count: u32,
    sum_x: f64,
    sum_y: f64,
    head: u32,
    children: [u32; 4],
}

impl Cell {
    fn new(cx: f64, cy: f64, half: f64) -> Self {
        Self {
            cx,
            cy,
            half,
            count: 0,
            sum_x: 0.0,
            sum_y: 0.0,
            head: NONE,
            children: [NONE; 4],
        }
    }

    /// The quadrant of `(x, y)`: bit 0 is high x, bit 1 is high y (`QuadTree_get_quadrant`).
    fn quadrant(&self, x: f64, y: f64) -> usize {
        usize::from(x - self.cx >= 0.0) | (usize::from(y - self.cy >= 0.0) << 1)
    }

    fn is_internal(&self) -> bool {
        self.children.iter().any(|&c| c != NONE)
    }
}

/// A built tree over borrowed positions. Cells live in one arena, so a walk is index arithmetic.
pub(super) struct Quadtree<'a> {
    cells: Vec<Cell>,
    /// `next[i]`: the point after `i` in its leaf's list.
    next: Vec<u32>,
    x: &'a [f64],
    y: &'a [f64],
}

impl<'a> Quadtree<'a> {
    /// A tree over `x`/`y`. The root is centred on the bounding box, with a half-width of 0.52
    /// times its larger span, floored at `1e-5` for a single point (`QuadTree.c:333-336`).
    pub(super) fn of(x: &'a [f64], y: &'a [f64]) -> Self {
        let (mut lo_x, mut hi_x) = (f64::MAX, f64::MIN);
        let (mut lo_y, mut hi_y) = (f64::MAX, f64::MIN);
        for i in 0..x.len() {
            lo_x = lo_x.min(x[i]);
            hi_x = hi_x.max(x[i]);
            lo_y = lo_y.min(y[i]);
            hi_y = hi_y.max(y[i]);
        }
        if x.is_empty() {
            (lo_x, hi_x, lo_y, hi_y) = (0.0, 0.0, 0.0, 0.0);
        }
        let half = (hi_x - lo_x).max(hi_y - lo_y).max(1e-5) * 0.52;
        let root = Cell::new((lo_x + hi_x) * 0.5, (lo_y + hi_y) * 0.5, half);
        let mut cells = Vec::with_capacity(2 * x.len() + 1);
        cells.push(root);
        let mut tree = Self {
            cells,
            next: vec![NONE; x.len()],
            x,
            y,
        };
        for i in 0..x.len() as u32 {
            tree.add(0, i, 0);
        }
        tree
    }

    /// `QuadTree_add_internal`: an empty cell takes the point; below the depth cap a cell
    /// splits and sends both the new point and the one it held down a level; at the cap it
    /// keeps a list.
    fn add(&mut self, node: usize, id: u32, level: u32) {
        let (px, py) = (self.x[id as usize], self.y[id as usize]);
        let cell = &mut self.cells[node];
        cell.count += 1;
        cell.sum_x += px;
        cell.sum_y += py;
        if cell.count == 1 || level >= MAX_DEPTH {
            self.next[id as usize] = cell.head;
            cell.head = id;
            return;
        }
        let held = std::mem::replace(&mut cell.head, NONE);
        self.add_below(node, id, level);
        if held != NONE {
            self.add_below(node, held, level);
        }
    }

    /// Sends `id` into its quadrant of `node`, creating that child on first use.
    fn add_below(&mut self, node: usize, id: u32, level: u32) {
        let (px, py) = (self.x[id as usize], self.y[id as usize]);
        let parent = &self.cells[node];
        let quadrant = parent.quadrant(px, py);
        let mut child = parent.children[quadrant];
        if child == NONE {
            let half = parent.half / 2.0;
            let cx = parent.cx + if quadrant & 1 == 0 { -half } else { half };
            let cy = parent.cy + if quadrant & 2 == 0 { -half } else { half };
            child = self.cells.len() as u32;
            self.cells.push(Cell::new(cx, cy, half));
            self.cells[node].children[quadrant] = child;
        }
        self.add(child as usize, id, level + 1);
    }

    /// The repulsive force on node `i`, `kp` being the reference's `K^(1-p)`.
    pub(super) fn repulsion(&self, i: u32, kp: f64) -> [f64; 2] {
        let mut out = [0.0f64; 2];
        let at = [self.x[i as usize], self.y[i as usize]];
        self.walk(0, i, at, kp, &mut out);
        out
    }

    /// `QuadTree_get_supernodes_internal` with the force summed as it goes: a cell's own
    /// points exactly, then the cell as one supernode if `half < bh · dist`, else its children.
    fn walk(&self, node: usize, i: u32, at: [f64; 2], kp: f64, out: &mut [f64; 2]) {
        let cell = &self.cells[node];
        let mut point = cell.head;
        while point != NONE {
            if point != i {
                let delta = [
                    at[0] - self.x[point as usize],
                    at[1] - self.y[point as usize],
                ];
                force::repel(out, delta, kp);
            }
            point = self.next[point as usize];
        }
        if !cell.is_internal() {
            return;
        }
        let (dx, dy) = (at[0] - cell.cx, at[1] - cell.cy);
        if cell.half < BH * f64::sqrt(dx * dx + dy * dy) {
            let weight = f64::from(cell.count);
            let delta = [at[0] - cell.sum_x / weight, at[1] - cell.sum_y / weight];
            force::repel(out, delta, weight * kp);
            return;
        }
        for &child in &cell.children {
            if child != NONE {
                self.walk(child as usize, i, at, kp, out);
            }
        }
    }

    /// The root square's side, for the module's own tests.
    #[cfg(test)]
    fn side(&self) -> f64 {
        2.0 * self.cells[0].half
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pairwise sum the tree approximates, for comparison.
    fn exact(x: &[f64], y: &[f64], i: usize, kp: f64) -> [f64; 2] {
        let mut out = [0.0, 0.0];
        for j in (0..x.len()).filter(|&j| j != i) {
            force::repel(&mut out, [x[i] - x[j], y[i] - y[j]], kp);
        }
        out
    }

    #[test]
    fn a_single_node_gets_a_finite_zero_repulsion() {
        let tree = Quadtree::of(&[1.0], &[2.0]);
        assert_eq!(
            tree.repulsion(0, 1.0),
            [0.0, 0.0],
            "a lone node has nothing to push against"
        );
    }

    /// Two nodes push each other apart, equally and oppositely. On 2026-10-01 each node was also
    /// pushed by its own leaf from that leaf's centre, so the two forces pointed anywhere.
    #[test]
    fn two_nodes_push_each_other_apart_equally() {
        let (x, y) = ([0.0, 2.0], [0.0, 0.0]);
        let tree = Quadtree::of(&x, &y);
        assert_eq!(
            (tree.repulsion(0, 1.0), tree.repulsion(1, 1.0)),
            ([-0.5, 0.0], [0.5, 0.0])
        );
    }

    /// Coincident points are what the depth cap exists for: the build terminates, and a
    /// coincident pair adds nothing rather than dividing by zero.
    #[test]
    fn coincident_points_stay_finite() {
        let (x, y) = ([0.0; 4], [0.0; 4]);
        let tree = Quadtree::of(&x, &y);
        for i in 0..4 {
            assert_eq!(tree.repulsion(i, 1.0), [0.0, 0.0], "node {i}");
        }
    }

    #[test]
    fn the_root_square_covers_the_span_with_a_margin() {
        let tree = Quadtree::of(&[0.0, 1.0], &[0.0, 1.0]);
        assert!((tree.side() - 1.04).abs() < 1e-12, "side {}", tree.side());
    }

    /// The approximation stays close to the exact sum on a spread-out cloud: supernodes sit at
    /// their centre of mass, so a far cluster pulls the way its points do. The error is measured
    /// against the gross force, the sum of every pair's magnitude, because the net force nearly
    /// cancels for a node inside the cloud: against the net, node 28 reads 36% off while its
    /// absolute error is 5.5% of what the pairs exert at worst (node 119). The reference's own criterion,
    /// `width < 0.6 * dist` on the cell's *half*-width, is theta 1.2 in side/distance terms.
    #[test]
    fn the_tree_agrees_with_the_exact_sum() {
        let x: Vec<f64> = (0..200)
            .map(|i| libm::sin(f64::from(i) * 1.7) * 10.0)
            .collect();
        let y: Vec<f64> = (0..200)
            .map(|i| libm::cos(f64::from(i) * 2.3) * 10.0)
            .collect();
        let tree = Quadtree::of(&x, &y);
        let mut total = 0.0;
        for i in 0..200 {
            let (a, b) = (tree.repulsion(i as u32, 1.0), exact(&x, &y, i, 1.0));
            let gross: f64 = (0..200)
                .filter(|&j| j != i)
                .map(|j| 1.0 / libm::hypot(x[i] - x[j], y[i] - y[j]))
                .sum();
            let error = libm::hypot(a[0] - b[0], a[1] - b[1]) / gross;
            assert!(
                error < 0.1,
                "node {i}: tree {a:?}, exact {b:?}, gross {gross}"
            );
            total += error;
        }
        assert!(total / 200.0 < 0.02, "mean error {}", total / 200.0);
    }

    /// Gathering twice gives the same answer: nothing accumulates across calls.
    #[test]
    fn repulsion_is_a_pure_function_of_the_positions() {
        let (x, y) = ([0.0, 1.0, 0.5, -1.0], [0.0, 1.0, -1.0, 0.5]);
        let tree = Quadtree::of(&x, &y);
        for i in 0..4 {
            assert_eq!(tree.repulsion(i, 1.0), tree.repulsion(i, 1.0), "node {i}");
        }
    }
}
