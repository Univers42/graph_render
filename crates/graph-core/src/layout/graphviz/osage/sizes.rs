//! The node box sizes the osage differential's fixtures pin, and the type the layout reads
//! them through.
//!
//! **Why this exists.** `arrayRects` sorts the rectangles by `width + height` with `qsort`
//! (`pack.c:569-579`, `pack.c:657-658`), and glibc's `qsort` is not stable, so among *tied*
//! boxes the cell order is the C library's choice rather than the graph's. The reference
//! also sizes each box from its rendered label, which graph-core has no font engine for.
//! Both were the two named causes of this differential disagreeing with Graphviz on 982 of
//! 1000 seeds (`docs/measurements/p13-gv1-osage.md`).
//!
//! The fixtures remove both at the source instead of widening a ceiling: every node gets an
//! explicit box, and `width + height` rises **strictly with the node index**, so no two
//! boxes tie, the descending sort is a total order, and the layout's `sort_by` is exact
//! rather than a guess about glibc. The DOT the harness writes pins the same numbers with
//! `fixedsize=true`, `width`, `height`, `label=""` and `margin=0`, so Graphviz sizes each
//! box from the attribute instead of from a rendered label.
//!
//! **The grid.** Every size is a whole number of `1/2048` inch — [`STEP`] is `9/256` point
//! and the two bases are `54 = 3/4` and `36 = 1/2` inch — and `points / 72 * 72` is that
//! same point on this grid exactly. A size that did not survive the round trip would make
//! the two arms size the same box differently, so the grid is a correctness requirement and
//! not tidiness; `tests.rs` measures the round trip rather than asserting it.

use super::POINTS_PER_INCH;

/// Graphviz's default `nodesize` in points: the box node `0` of every table carries, and
/// the size the whole uniform grid (`grid.rs`) is built from.
const BASE_W: f64 = 0.75 * POINTS_PER_INCH;
const BASE_H: f64 = 0.5 * POINTS_PER_INCH;

/// The size step, in points: `9/256` point is exactly `1/2048` inch, so every box this
/// module builds is on a grid that survives the inch round trip bit for bit.
const STEP: f64 = 9.0 / 256.0;

/// How many nodes share one height step before the table steps up. Eight widths and then
/// eight heights is the ladder that makes `width + height` rise with the index; the number
/// itself is free past that, and 8 keeps a 601-node graph's tallest box inside two inches.
const LADDER: usize = 8;

/// One node's box, in points. Named rather than reusing the prelude's `Box` so a reader
/// meeting it in [`Boxes`] cannot mistake it for the standard library's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeBox {
    /// Box width in points, before the reference's margin is added.
    pub w: f64,
    /// Box height in points, before the reference's margin is added.
    pub h: f64,
}

impl NodeBox {
    /// Graphviz's default `nodesize` box, `0.75 x 0.5` inch in points: the size every node
    /// has below eleven in a label-sized graph, and the size the uniform grid is built from.
    pub const DEFAULT: NodeBox = NodeBox {
        w: BASE_W,
        h: BASE_H,
    };
}

/// One box per node, in dense index order — the reference's `gs` array, in the shape the
/// layout reads. Its own type so no call can pass a width column where a table belongs.
#[derive(Clone, Debug, PartialEq)]
pub struct Boxes {
    boxes: Vec<NodeBox>,
}

impl Boxes {
    /// A table of boxes in dense index order.
    pub fn new(boxes: Vec<NodeBox>) -> Self {
        Self { boxes }
    }

    /// `count` copies of one box: the uniform table, which is what a graph whose every node
    /// ties is packed from.
    pub fn uniform(count: u32, node_box: NodeBox) -> Self {
        Self::new(vec![node_box; count as usize])
    }

    /// The fixture table for a graph of `count` nodes. Node `index` always gets
    /// [`Self::box_at`]`(index)`, whatever `count` is, so a seed's answer does not depend on
    /// how many nodes its graph happens to have.
    pub fn table(count: u32) -> Self {
        Self::new((0..count).map(Self::box_at).collect())
    }

    /// Node `index`'s box in points.
    ///
    /// `width + height` is `BASE_W + BASE_H + STEP * index` for every node, which is the
    /// whole point: strictly increasing, so the sort `acmpf` performs never ties and never
    /// falls back on the C library's tie order.
    pub fn box_at(index: u32) -> NodeBox {
        let ladder = index as usize % LADDER;
        let riser = index as usize / LADDER;
        NodeBox {
            w: BASE_W + STEP * ladder as f64,
            h: BASE_H + LADDER as f64 * STEP * riser as f64,
        }
    }

    /// The table, one box per node in dense index order.
    pub fn get(&self) -> &[NodeBox] {
        &self.boxes
    }

    /// How many nodes the table covers, which is also the node count it is valid for.
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    /// Whether the table covers no node at all.
    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }
}
