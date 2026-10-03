//! An iterative point quadtree over the dense position arrays, structurally ported from
//! d3-quadtree (`/home/user/refs/npm/d3-quadtree-3.0.1/src/{add,cover,visit,visitAfter}.js`,
//! pinned in `node_modules`): the same bounding-square growth (`cover`), the same
//! per-point insertion (`add`, already iterative — a `while` loop, no recursion), the same
//! exact-coincidence chaining. A build ends by flattening the pointer tree into a
//! preorder arena ([`Cell`], `preorder.rs`), which is the only form a pass walks: no
//! stack, no recursion (devil C11: wasm32's default stack is far smaller than native's),
//! and the cells in the order every walk reads them. Every `Vec` here is `clear()`-ed and
//! refilled, never reallocated once its capacity reaches steady state (`dsa-and-memory.md`).

mod bounds;
mod build;
mod preorder;

pub(crate) use bounds::Bounds;
use build::Builder;
pub(crate) use preorder::Cell;
use preorder::Visit;

/// One arena slot: an internal node's up to 4 children, or a leaf's chain head.
#[derive(Debug, Clone, Copy)]
enum Shape {
    Internal([Option<u32>; 4]),
    Leaf(u32),
}

/// The point arrays, bundled so build methods take at most four parameters besides the
/// receiver (`prompt.md` §0's cap, counted the way `builder.add` already needs it).
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

/// The finite points' extent, `(x0, y0, x1, y1)` (d3's `addAll`, `add.js:61-70`);
/// a non-finite point is excluded, then ignored again on insertion (`add.js:8`),
/// matching d3. `±inf` is refused with the `NaN` it used to be admitted beside: it
/// made `cover` grow a square that can never contain it and `insert_leaf` split for
/// ever (`LF-02`).
fn bounds_of(pts: Points<'_>) -> Option<(f64, f64, f64, f64)> {
    let ok = |(&x, &y): (&f64, &f64)| (x.is_finite() && y.is_finite()).then_some((x, y));
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
    cells: Vec<Cell>,
    node_id: Vec<u32>,
    order: Vec<u32>,
    pending: Vec<Visit>,
}

impl Quadtree {
    /// Number of cells (internal or leaf) after the last [`build`](Self::build).
    #[cfg(test)]
    pub(crate) fn len(&self) -> u32 {
        self.cells.len() as u32
    }

    /// The cells in preorder, children in slot order `0,1,2,3` (d3's `visit.js` order).
    pub(crate) fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Cell `k`'s shape-arena slot: the pointer-tree node the arena was flattened from.
    /// It is **not** a point index and not the insertion order — the two diverge as soon
    /// as a second split happens, which `quadtree/tests.rs`'s
    /// `a_cell_s_node_id_is_a_shape_slot_and_never_a_point_index` pins. It is the key the
    /// opening-angle jiggle has always hashed, kept so the preorder arena moves no byte.
    pub(crate) fn node_id(&self, k: u32) -> u32 {
        self.node_id[k as usize]
    }

    /// Every point, leaf by leaf in preorder, each leaf's chain most-recent-first; the
    /// points with a NaN coordinate (in no leaf) follow, ascending. A cell's own points are
    /// `order[cell.start..cell.end]`.
    pub(crate) fn order(&self) -> &[u32] {
        &self.order
    }

    /// The up to 4 children of an internal node, empty slots `None`; `None` for a leaf.
    fn children(&self, node: u32) -> Option<[Option<u32>; 4]> {
        match self.shape[node as usize] {
            Shape::Internal(c) => Some(c),
            Shape::Leaf(_) => None,
        }
    }

    /// A leaf's chain head, the most recently inserted of its coincident points.
    fn head(&self, node: u32) -> Option<u32> {
        match self.shape[node as usize] {
            Shape::Leaf(h) => Some(h),
            Shape::Internal(_) => None,
        }
    }

    /// Rebuilds over `xs`/`ys` — equal lengths, both short enough for the `u32` point
    /// index, or the build is refused and the tree is left empty — point `i` inserted
    /// ascending, fixed, deterministic order. Clears and refills every buffer.
    pub(crate) fn build(&mut self, xs: &[f64], ys: &[f64]) {
        self.reset();
        // Ponytail: a mismatch and a count past `u32::MAX` are both refused as an empty
        // tree, not as a `StageError` — `build` has no error channel and its callers are
        // three modules wide, so the refusal is a silent empty arena where every walk
        // contributes nothing instead of a panic or a wrapped index. Escape hatch:
        // `force::planar_points` refuses a non-finite column before any tick, and the
        // stage's own post-run check turns a mid-run overflow into `StageError`.
        if xs.len() != ys.len() || u32::try_from(xs.len()).is_err() {
            return;
        }
        self.chain_next.resize(xs.len(), None);
        let pts = Points { xs, ys };
        let Some((x0, y0, x1, y1)) = bounds_of(pts) else {
            self.flatten(pts);
            return;
        };
        self.cover(x0, y0);
        self.cover(x1, y1);
        let mut builder = Builder { tree: self, pts };
        for i in 0..xs.len() as u32 {
            builder.add(i);
        }
        self.flatten(pts);
    }

    /// Every buffer a build refills, cleared: a refused build leaves an empty arena rather
    /// than the previous tick's.
    fn reset(&mut self) {
        self.shape.clear();
        self.chain_next.clear();
        self.cells.clear();
        self.node_id.clear();
        self.order.clear();
        self.pending.clear();
        self.root = None;
        self.root_bounds = Bounds::default();
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
            // "Stopped growing" has to mean *permanently*: a coordinate past 2^53 makes
            // `b.x0 + z` round back to `b.x0`, so the span stays 0 for the first ~57
            // doublings and only starts moving once `z` clears it. The dead end is `z`
            // itself overflowing, or the span going `NaN` — both leave a square no
            // comparison can grow.
            //
            // Ponytail: a bail leaves the square as it stands, so a point outside it lands
            // in whichever quadrant its sign picks — wrong for that one point, bounded for
            // the build, where the loop it replaces spun and allocated forever. Escape
            // hatch: `build` refuses the non-finite coordinates that reach this, so the
            // bail is the backstop for the ones it cannot.
            if !(z.is_finite() && b.span().is_finite()) {
                return;
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

    /// The capacity of every buffer a rebuild refills, for the test that a rebuild
    /// allocates nothing once the layout has reached steady state.
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.shape.capacity()
            + self.chain_next.capacity()
            + self.cells.capacity()
            + self.node_id.capacity()
            + self.order.capacity()
            + self.pending.capacity()
    }
}

#[cfg(test)]
mod tests;
