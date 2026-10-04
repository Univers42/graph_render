//! An iterative point octree over the dense three-axis position arrays, the 3D
//! counterpart of `quadtree.rs` — deliberately a **second** tree rather than a generalised
//! one.
//!
//! Why not generalise: the 2D quadtree's `node_id` is the key the many-body walk's
//! opening-angle jiggle has hashed since the session refactor, and the arena it flattens
//! into is pinned byte-for-byte by the 65 frozen force-session digests
//! (`force/session/tests/golden.rs`) and the 224-line 2D golden pin
//! (`harness/golden-2d-before.txt`). A branch factor that became a parameter would move
//! the slot arithmetic in every one of those, so this module is written beside the
//! quadtree, in the quadtree's own shape, and the two share no line of build code.
//!
//! What is copied and why: d3-quadtree has no 3D port, so this is the quadtree's algorithm
//! with an octant instead of a quadrant — the same `cover` growth of a cube from the
//! finite extent, the same per-point insertion in ascending point order (already a `while`
//! loop, no recursion), the same exact-coincidence chaining, and the same flatten into a
//! preorder arena (`preorder.rs`) that is the only form a pass walks: no stack, no
//! recursion (devil C11: wasm32's default stack is far smaller than native's). Every
//! `Vec` here is `clear()`-ed and refilled, never reallocated once its capacity reaches
//! steady state (`dsa-and-memory.md`).
//!
//! **The slot index is three bits**, `z<<2 | y<<1 | x`, where each bit is "at or above this
//! axis' midpoint". The quadtree's is two, `y<<1 | x`. That single difference is what makes
//! this a different tree rather than a re-spelling: the same point set subdivides along a
//! different axis each level, so the arena, the cell order and `node_id` all differ — which
//! is why the 3D arms can have their own bytes and the 2D ones keep theirs.

pub(crate) mod charge;
mod bounds;
mod build;
mod preorder;
#[cfg(test)]
mod tests;

pub(crate) use bounds::Bounds3;
use build::Builder;
pub(crate) use preorder::Cell;
use preorder::Visit;

/// Children per internal node, and the width of the slot index a `narrow` returns.
pub(crate) const OCTANTS: usize = 8;

/// One arena slot: an internal node's up to [`OCTANTS`] children, or a leaf's chain head.
#[derive(Debug, Clone, Copy)]
enum Shape {
    Internal([Option<u32>; OCTANTS]),
    Leaf(u32),
}

/// The three position columns, bundled so build methods take at most four parameters
/// besides the receiver (`prompt.md` §0's cap) — the quadtree's `Points` at one more axis.
#[derive(Clone, Copy)]
struct Points3<'a> {
    xs: &'a [f64],
    ys: &'a [f64],
    zs: &'a [f64],
}

impl Points3<'_> {
    fn at(&self, i: u32) -> (f64, f64, f64) {
        let i = i as usize;
        (self.xs[i], self.ys[i], self.zs[i])
    }

    fn len(&self) -> usize {
        self.xs.len()
    }
}

/// The finite points' extent, `(x0, y0, z0, x1, y1, z1)`; a non-finite point is excluded,
/// then ignored again on insertion, matching `quadtree.rs`'s `bounds_of` rule for the same
/// reason: `±inf` would make `cover` grow a cube that can never contain it and
/// `insert_leaf` split for ever.
fn bounds_of(pts: Points3<'_>) -> Option<(f64, f64, f64, f64, f64, f64)> {
    let ok = |(&x, &y, &z): (&f64, &f64, &f64)| {
        (x.is_finite() && y.is_finite() && z.is_finite()).then_some((x, y, z))
    };
    let mut valid = pts.xs.iter().zip(pts.ys).zip(pts.zs).map(|((&x, &y), &z)| (x, y, z)).filter_map(ok);
    let (fx, fy, fz) = valid.next()?;
    Some(valid.fold((fx, fy, fz, fx, fy, fz), |(a, b, c, d, e, f), (x, y, z)| {
        (a.min(x), b.min(y), c.min(z), d.max(x), e.max(y), f.max(z))
    }))
}

/// A reused Barnes-Hut octree: rebuild every tick with [`build`](Octree::build), no
/// per-tick allocation once buffers reach steady-state capacity.
#[derive(Debug, Clone, Default)]
pub(crate) struct Octree {
    root_bounds: Bounds3,
    shape: Vec<Shape>,
    root: Option<u32>,
    chain_next: Vec<Option<u32>>,
    cells: Vec<Cell>,
    node_id: Vec<u32>,
    order: Vec<u32>,
    pending: Vec<Visit>,
}

impl Octree {
    /// Number of cells (internal or leaf) after the last [`build`](Self::build).
    #[cfg(test)]
    pub(crate) fn len(&self) -> u32 {
        self.cells.len() as u32
    }

    /// The cells in preorder, children in slot order `0..7` (the quadtree's `visit.js`
    /// order with one more bit).
    pub(crate) fn cells(&self) -> &[Cell] {
        &self.cells
    }

    /// Cell `k`'s shape-arena slot: the pointer-tree node the arena was flattened from. It
    /// is **not** a point index and not the insertion order, exactly as in the quadtree,
    /// where the two diverge as soon as a second split happens. It is the key the 3D
    /// opening-angle jiggle hashes (`charge.rs`), so it is kept even though nothing has
    /// ever hashed it before.
    pub(crate) fn node_id(&self, k: u32) -> u32 {
        self.node_id[k as usize]
    }

    /// Every point, leaf by leaf in preorder, each leaf's chain most-recent-first; the
    /// points with a non-finite coordinate (in no leaf) follow, ascending. A cell's own
    /// points are `order[cell.start..cell.end]`.
    pub(crate) fn order(&self) -> &[u32] {
        &self.order
    }

    /// The up to [`OCTANTS`] children of an internal node, empty slots `None`; `None` for
    /// a leaf.
    fn children(&self, node: u32) -> Option<[Option<u32>; OCTANTS]> {
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

    /// Rebuilds over `xs`/`ys`/`zs` — equal lengths, all short enough for the `u32` point
    /// index, or the build is refused and the tree is left empty — point `i` inserted
    /// ascending, fixed, deterministic order. Clears and refills every buffer.
    ///
    /// Ponytail: a mismatch and a count past `u32::MAX` are refused as an empty tree, not
    /// as a `StageError` — `build` has no error channel, so the refusal is a silent empty
    /// arena where every walk contributes nothing instead of a panic. Escape hatch: the
    /// stage's own post-run check turns a non-finite column into `StageError::NonFinite`.
    pub(crate) fn build(&mut self, pts: Points3<'_>) {
        unimplemented!("RED: build")
    }

    /// Every buffer a build refills, cleared: a refused build leaves an empty arena rather
    /// than the previous tick's.
    fn reset(&mut self) {
        unimplemented!("RED: reset")
    }

    /// Grows the root cube to cover `(x, y, z)`. Only [`build`](Self::build) calls it,
    /// always on an empty tree, so it never has to re-root an existing cube the way the
    /// quadtree's never does.
    fn cover(&mut self, p: (f64, f64, f64)) {
        unimplemented!("RED: cover")
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