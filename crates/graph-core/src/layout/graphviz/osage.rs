//! Graphviz 16.1.0's `osage`: array packing, a rectangle grid of node boxes.
//!
//! Reference: `lib/osage/osageinit.c` and `lib/pack/pack.c` (`arrayRects`) of the pinned
//! Graphviz 16.1.0 release, read as an algorithm reference and reimplemented (never
//! translated, never linked — `docs/decisions/graphviz-oracle.md`). The steps below are
//! the reference's, in its order:
//!
//! 1. `mkClusters` + `layout` recursion — clusters are packed first, recursively, and each
//!    cluster's box becomes one rectangle in its parent's array. **This port has no
//!    clusters**: the motor's [`Topology`] is a flat node set, so the recursion has depth
//!    one and the whole graph is a single array.
//! 2. `getPackInfo(g, l_array, DFLT_MARGIN)` — array packing with four points of margin
//!    around every rectangle, no alignment flag and no row/column count.
//! 3. `putRects` → `arrayRects` — a near-square grid, filled **row by row in the order
//!    the nodes were declared**.
//! 4. `ND_coord(n) = mid_pointf(bb.LL, bb.UR)` — each node at its own box's centre.
//! 5. `osageinit.c:198-220` — the whole drawing is translated so its lower-left corner is
//!    the origin, and the node coordinates go with it.
//!
//! **Units are points, and the translation is kept.** Graphviz computes in inches and
//! reports `72 *` them, and step 5 is part of the answer rather than a presentation
//! detail: the port emits exactly the coordinates `-Tplain` prints, scaled by 72, so the
//! small closed cases agree with the oracle **byte for byte** with no offset applied on
//! either side. The differential's bounding-box rescale then removes the translation
//! again for the 1000-seed metric, as it does for every arm.
//!
//! **The edges are not read at all.** `arrayRects` takes a list of rectangles and returns
//! a list of offsets; the graph's edges never enter it, and the reference lays out a
//! `graph g { n0; n1; n2; }` and a `graph g { n0 -- n1; n1 -- n2; }` identically. So the
//! neighbour order that binds `layout::radial::twopi` is not load-bearing here. What *is*
//! load-bearing is the node count **and the node ids**, because a node's box is sized from
//! its rendered label — see the first Ponytail marker below, which is where the agreement
//! with Graphviz ends.
//!
//! Determinism: `-Gstart` is INERT for this engine (measured — `docs/measurements/
//! p13-gv1-osage.md`), the engine is closed form, and the only arithmetic is the exact
//! `f64` multiply and add of `grid`, whose two `libm` calls are the reference's own
//! `ceil(sqrt(n))`. The kernel **is** in gather form (D10): node `i`'s box depends only on
//! `i` and the node count and never on a neighbour, so one ordered pass places the grid.
//! There is no reduction whose order could move the bytes except the origin fold, which is
//! a `min` written in dense-index order.
//!
//! Ponytail: **node box size, and it is the one that costs the agreement.** Every rectangle
//! is taken to be Graphviz's default `nodesize` of 0.75 x 0.5 inch, and that is exact only
//! while every node's *label* fits inside the minimum. It stops being exact at eleven
//! nodes: `-Tplain` reports a 54 point box for `n0`..`n9` and **57.942** for `n10`..`n99`,
//! because a three-character label grows the box past the default, and `arrayRects` takes
//! each column's cell width from the widest box in it. Failing input: any graph of eleven
//! or more nodes whose ids carry three or more characters — which is every graph past the
//! tenth here. Direction: a *different drawing*, not a worse one: our grid is uniform
//! 58 x 40 where the reference's columns vary, so rows and columns drift apart by up to
//! 1785 points over the 1000-seed sweep (measured, `docs/measurements/p13-gv1-osage.md`).
//! Escape hatch: none inside the motor — the width is a font metric of Graphviz's own text
//! layout, and graph-core has no font engine and `layout` emits `Point` geometry with no
//! box at all. What *is* held exactly is everything below eleven nodes, where all boxes
//! tie: the six closed cases agree with the oracle **byte for byte**, and the sweep gap
//! there is 3.6e-3 points, the oracle's own printed quantum.
//!
//! Ponytail: **`qsort` tie order.** `arrayRects` sorts the rectangles by `width + height`
//! and glibc's `qsort` is not stable, so which node lands in which cell among *tied* boxes
//! is the C library's choice, not the graph's. It is declaration order for every size
//! measured here, and the port assumes it. Failing input: a size where glibc's introsort
//! permutes the ties. Direction: a permuted drawing. Escape hatch: none, and none needed —
//! below eleven nodes every box ties *and* they are all the same size, so the sort cannot
//! move the geometry, only the names, and the names are in declaration order.
//!
//! Ponytail: **`pack`, `packmode` and `nodesize` attributes.** The reference reads all
//! three and this port reads none, because the motor publishes no attribute channel to a
//! layout at its default parameters (the same decision `layout::radial::twopi` records).
//! Failing input: a Graphviz graph carrying `packmode="node"` or a `nodesize`. Direction:
//! this port packs the array the reference's defaults pack, which is the default answer.
//! Escape hatch: a `Params` on the stage, which is a contract change and not this job's.

mod array;
mod grid;
mod sizes;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;

#[cfg(test)]
mod tests;

use grid::Grid;
pub use sizes::{Boxes, NodeBox};

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.packing.osage";

/// `POINTS_PER_INCH` (`lib/common/geom.h:58`): the reference computes in inches and reports
/// points, so this is the only conversion the whole layout needs — and the differential's
/// fixture crosses it in the other direction, so it is public rather than module-private.
pub const POINTS_PER_INCH: f64 = 72.0;

/// Graphviz's default `nodesize` (0.75 x 0.5 inch) in points: the box every node is given
/// by `gv_nodesize` **while its label fits inside it**, which is the Ponytail case above —
/// measured to stop holding at eleven nodes, where `n10`'s box is 57.942 points wide.
const NODE_W: f64 = 0.75 * POINTS_PER_INCH;
const NODE_H: f64 = 0.5 * POINTS_PER_INCH;

/// `DFLT_MARGIN` (`lib/neatogen/adjust.h:24`): four points, the value `osageinit.c:96`
/// hands `getPackInfo` because the graph sets no `pack` attribute.
const MARGIN: f64 = 4.0;

/// The cell stride `arrayRects` measures with: a rectangle's own extent plus the margin
/// that goes around it (`pack.c:645-646`, `ip->width = bb.UR.x - bb.LL.x + pinfo->margin`).
pub(super) const CELL_W: f64 = NODE_W + MARGIN;
pub(super) const CELL_H: f64 = NODE_H + MARGIN;

/// The two output columns, sized once and filled in place.
///
/// Its own type rather than two `&mut Vec<f64>` so no call can hand the same column twice,
/// and so [`Grid::fill`] takes one argument instead of three.
pub(super) struct Columns {
    x: Vec<f64>,
    y: Vec<f64>,
}

impl Columns {
    /// Two zeroed columns, one slot per node.
    fn new_columns(count: u32) -> Self {
        Self {
            x: vec![0.0; count as usize],
            y: vec![0.0; count as usize],
        }
    }

    /// The point layout's columns, narrowed once at the end.
    pub(super) fn geometry(self) -> Geometry {
        point_geometry(&self.x, &self.y)
    }

    /// Two zeroed columns, so the sized path fills them the same way.
    pub(super) fn new(count: u32) -> Self {
        Self::new_columns(count)
    }

    /// One node's centre, in points.
    fn set(&mut self, node: u32, x: f64, y: f64) {
        self.x[node as usize] = x;
        self.y[node as usize] = y;
    }

    /// The two columns together, so the two `run` paths apply the origin fold the same way.
    pub(super) fn columns_mut(&mut self) -> (&mut [f64], &mut [f64]) {
        (&mut self.x, &mut self.y)
    }

    /// The node count, which is this column pair's length.
    fn len(&self) -> u32 {
        self.x.len() as u32
    }
}

/// `layout.packing.osage` at its defaults. No `Stage` impl and no `Params`, by the same
/// decision `layout::radial::twopi` records: the reference exposes `pack`, `packmode` and
/// `nodesize`, none of which the ledger row or the oracle sets, so the layout is a pure
/// function of the node count and publishing a `Params` would buy a knob with nothing
/// behind it. `GM_MUTATE_TWOPI_NODES` re-draws the twopi stage's own model; there is no
/// osage knob, because `hashgate/knob.rs` is shared with the parallel Graphviz engine jobs.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    let grid = Grid::of(count);
    let mut out = Columns::new_columns(count);
    let low = grid.fill(&mut out);
    let (xs, ys) = out.columns_mut();
    for (x, y) in xs.iter_mut().zip(ys) {
        *x -= low.0;
        *y -= low.1;
    }
    Ok(out.geometry())
}

/// `layout.packing.osage` with **explicit per-node boxes**: the reference's `arrayRects`
/// over boxes that differ, which is what [`run`] collapses when every box is the same size.
///
/// This is the narrow per-node size input the layout needed and did not have. `run` is a
/// pure function of the node count, because every rectangle was Graphviz's default
/// `nodesize`; the reference sizes each rectangle from **its own node** (`osageinit.c:124`,
/// `ND_xsize(n)`), and that is a font metric of Graphviz's text layout, which graph-core has
/// no engine for. The two callers are deliberately different:
///
/// - **the ledger row** runs [`run`] — the default answer, a pure function of the node
///   count, unchanged by this entry point and by the fixtures;
/// - **the differential** runs this one, over [`Boxes::table`], and the DOT the harness feeds
///   Graphviz pins the same sizes with `fixedsize=true` (`docs/measurements/p13-gv1-osage.md`).
///
/// Refuses a table that is not one box per node, rather than reading a truncated or over-long
/// table as this graph's answer.
pub fn run_sized(topology: &Topology, boxes: &Boxes) -> Result<Geometry, StageError> {
    array::pack(topology, boxes)
}
