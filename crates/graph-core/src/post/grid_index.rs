//! A uniform grid over the node geometry: which cell a coordinate is in, and which cells
//! a node occupies. The shared spatial substrate of Phase 8's POST stage
//! (`prompts/phase-08-post-routing-bundling.md` step 2): [`super::routed`] routes over
//! it, and Phase 9's LOD reuses it as a screen.
//!
//! Reference: `SciGraphs/engine/scigraphs_engine/bundling/routed.py` (`_grid`,
//! `_cell_of`) — a cubic uniform grid over the node coordinates with a `margin` of empty
//! cells around the extent, indexed with **x fastest**.
//!
//! # What is ported, and what is not
//!
//! Ported: the cubic cell size (`span / resolution`, the span being the larger of the two
//! axes so cells stay square), the `margin` of empty cells around the drawing, the flat
//! cell index with x fastest (`_cell_of`'s `strides`), and occupancy by node footprint.
//!
//! Not ported, deliberately: the reference's `_frame` rotates into the graph's own
//! principal plane, which exists because SciGraphs routes in 3D — the motor's node
//! geometry is 2D, so coordinates are used as they lie. The reference's `density_cost`
//! and `_reinforced` are *soft* per-cell costs that make routes share cells, which is
//! edge bundling and belongs to `post::fdeb` / `post::mingle`; this grid marks cells as
//! obstacles and nothing else.
//!
//! # The boundary tie-break
//!
//! Cells on an axis are the half-open intervals `[origin + k·cell, origin + (k+1)·cell)`.
//! A coordinate landing **exactly** on a boundary therefore belongs to the cell that
//! boundary opens — the higher index — and never to the one it closes. [`axis_cell`]
//! applies that rule and snaps the quotient to the nearest whole number when it is within
//! [`BOUNDARY_TOL`] cells of one, so the rule survives the rounding of a floating-point
//! division. Without the snap a node on a boundary would be assigned to whichever side
//! the division happened to round to, which is exactly the ambiguity the phase asks to be
//! made explicit.
//!
//! Ponytail (boundary): the snap is a tolerance, not an exactness. Failing input: a node
//! placed less than `BOUNDARY_TOL` of a cell *below* a boundary, which the snap pulls onto
//! the boundary and so one cell higher than exact arithmetic would place it. Direction:
//! the node's obstacle covers one extra cell — a hair more clearance, never less, so a
//! route never grazes a node it should have cleared. Escape hatch: `BOUNDARY_TOL` is a
//! named constant in this module; set it to 0 for strict `floor` behaviour.
//!
//! Ponytail (resolution): `resolution` is cells across the **longer** axis, so cells grow
//! quadratically with it and a gap between two nodes can be narrower than one cell. The
//! failure that causes — a route that detours, or gives up and draws through a node — is
//! reported by [`super::routed`], which is where the flag that reports it lives.

use crate::stage::StageError;
use graph_contract::geometry::NodeGeometry;

#[cfg(test)]
mod tests;

/// How close, in cells, a quotient must be to a whole number for [`axis_cell`] to treat
/// it as landing on that boundary. See the module's Ponytail (boundary).
pub const BOUNDARY_TOL: f64 = 1e-9;

/// The grid's parameters. Its defaults are conventions, pinned by a snapshot hash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridParams {
    /// Cells across the **longer** axis of the node bounding box, at least 1.
    pub resolution: u32,
    /// Empty cells added on every side of the drawing, so a route can bow outside the hull.
    pub margin: u32,
    /// Extra cells of clearance added around every node's footprint, as a multiple of the
    /// cell size — **0 by default, and that default is load-bearing.**
    ///
    /// A *whole* cell of clearance seals every node: a node's own cell plus the eight
    /// around it is a solid 3 × 3 block, so nothing can ever step out of it and every edge
    /// falls back. Measured, not assumed (`docs/measurements/phase08-routing.md` §3): at
    /// `clearance: 1` **every** edge of every layout tried reported a fallback, from 12
    /// nodes at 0.9% of cells blocked to 992 nodes at 52%. The knob is here because a
    /// caller with a node radius wants the route to keep further off — but at a whole cell
    /// it disables the capability, so it is not the default.
    pub clearance: f64,
}

impl Default for GridParams {
    /// The reference's `routed_resolution` (128) and `MARGIN` (2), and no clearance: a
    /// node blocks its own cell and nothing more, which is what "nodes as obstacles" means
    /// at the resolution the grid is drawn at. See [`GridParams::clearance`] for why the
    /// obvious value of 1 is wrong.
    fn default() -> Self {
        Self {
            resolution: 128,
            margin: 2,
            clearance: 0.0,
        }
    }
}

/// A uniform grid over a node layout: the cell arithmetic, and which cells are occupied.
///
/// Built into a **reused buffer** (`dsa-and-memory.md`): [`GridIndex::build`] refills the
/// allocation it already holds, so routing a second layout over the same index allocates
/// nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GridIndex {
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    nx: u32,
    ny: u32,
    /// One byte per cell, `1` when a node's footprint (plus clearance) covers it.
    occupied: Vec<u8>,
    /// One cell per node, in node order: the cell its centre is in.
    node_cell: Vec<u32>,
}

impl GridIndex {
    /// An empty grid: no cells, no nodes. Every query on it answers 0.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of cells, `nx * ny`.
    pub fn cells(&self) -> u32 {
        self.nx * self.ny
    }

    /// Cells along x, then along y.
    pub fn shape(&self) -> (u32, u32) {
        (self.nx, self.ny)
    }

    /// Side of a cell, in layout units. 0 for an empty grid.
    pub fn cell_size(&self) -> f64 {
        self.cell
    }

    /// `(x, y)` of the cell's centre, in layout units. `(0.0, 0.0)` for an empty grid,
    /// which has no cell to place.
    pub fn centre(&self, cell: u32) -> (f64, f64) {
        if self.cells() == 0 {
            return (0.0, 0.0);
        }
        let (ix, iy) = self.xy(cell);
        (
            self.origin_x + (f64::from(ix) + 0.5) * self.cell,
            self.origin_y + (f64::from(iy) + 0.5) * self.cell,
        )
    }

    /// True when a node's footprint covers `cell`. False for a cell out of range.
    pub fn is_occupied(&self, cell: u32) -> bool {
        cell < self.cells() && self.occupied[cell as usize] == 1
    }

    /// Node `node`'s cell. 0 for a node index past the built layout.
    pub fn node_cell(&self, node: u32) -> u32 {
        self.node_cell.get(node as usize).copied().unwrap_or(0)
    }

    /// Number of nodes the index was built over.
    pub fn node_count(&self) -> u32 {
        self.node_cell.len() as u32
    }

    /// Number of occupied cells.
    pub fn occupied_cells(&self) -> u32 {
        self.occupied.iter().filter(|c| **c == 1).count() as u32
    }

    /// Bytes the reused buffers hold, for the memory measurement.
    pub fn bytes(&self) -> usize {
        self.occupied.capacity() * size_of::<u8>() + self.node_cell.capacity() * size_of::<u32>()
    }

    /// The cell holding `(x, y)`, by the module's boundary rule. 0 on an empty grid.
    pub fn cell_of(&self, x: f64, y: f64) -> u32 {
        if self.cells() == 0 {
            return 0;
        }
        let ix = axis_cell(x, self.origin_x, self.cell, self.nx);
        let iy = axis_cell(y, self.origin_y, self.cell, self.ny);
        iy * self.nx + ix
    }

    /// `(ix, iy)` of `cell`, x fastest. `(0, 0)` for an empty grid.
    pub fn xy(&self, cell: u32) -> (u32, u32) {
        if self.nx == 0 {
            return (0, 0);
        }
        (cell % self.nx, cell / self.nx)
    }

    /// The raw occupancy bytes, one per cell, for the CSR build in [`super::routed`].
    pub fn occupancy(&self) -> &[u8] {
        &self.occupied
    }

    /// (Re)builds the index over `geometry` at `params`, refilling the buffers it already
    /// holds — the reused buffer `dsa-and-memory.md` asks for, so routing a second layout
    /// over the same index allocates nothing.
    ///
    /// Refused rather than producing a NaN cell or a zero cell size: a non-finite
    /// coordinate (D9 — its bits are not pinned across targets) and a zero `resolution`.
    /// An empty layout gives an empty grid and is **not** an error: a graph with no nodes
    /// has no cells to route over.
    pub fn build(
        &mut self,
        geometry: &NodeGeometry,
        params: &GridParams,
    ) -> Result<(), StageError> {
        check_finite(geometry)?;
        if params.resolution == 0 {
            return Err(StageError::Param {
                name: "resolution",
                rule: "at least 1",
            });
        }
        let boxes = footprints(geometry);
        self.origin_x = 0.0;
        self.origin_y = 0.0;
        self.cell = 0.0;
        self.nx = 0;
        self.ny = 0;
        self.occupied.clear();
        self.node_cell.clear();
        if boxes.is_empty() {
            return Ok(());
        }
        let (cell, nx, ny) = axes(&boxes, params);
        self.cell = cell;
        self.nx = nx;
        self.ny = ny;
        let (lx, _, ly, _) = bounds(&boxes);
        self.origin_x = lx - f64::from(params.margin) * cell;
        self.origin_y = ly - f64::from(params.margin) * cell;
        self.occupied.resize((nx * ny) as usize, 0);
        self.mark(&boxes, params.clearance);
        // Refilled in place, not reassigned: a `collect` would drop the old allocation and
        // the second layout would pay for a fresh one, which is the churn the reused
        // buffer exists to avoid.
        self.node_cell.clear();
        self.node_cell.extend(boxes.iter().map(|b| {
            let ix = axis_cell(b.0, self.origin_x, cell, nx);
            let iy = axis_cell(b.2, self.origin_y, cell, ny);
            iy * nx + ix
        }));
        Ok(())
    }

    /// Marks every cell a footprint's inflated box reaches, under the half-open boundary
    /// rule: a cell is occupied when the box overlaps the half-open interval
    /// `[origin + k·cell, origin + (k+1)·cell)`.
    fn mark(&mut self, boxes: &[(f64, f64, f64, f64)], clearance: f64) {
        let pad = clearance * self.cell;
        for (x0, x1, y0, y1) in boxes {
            let (ax, bx) = (
                axis_cell(x0 - pad, self.origin_x, self.cell, self.nx),
                axis_cell(x1 + pad, self.origin_x, self.cell, self.nx),
            );
            let (ay, by) = (
                axis_cell(y0 - pad, self.origin_y, self.cell, self.ny),
                axis_cell(y1 + pad, self.origin_y, self.cell, self.ny),
            );
            for iy in ay..=by {
                for ix in ax..=bx {
                    self.occupied[(iy * self.nx + ix) as usize] = 1;
                }
            }
        }
    }
}

/// The node bounding box `(min_x, max_x, min_y, max_y)`.
fn bounds(boxes: &[(f64, f64, f64, f64)]) -> (f64, f64, f64, f64) {
    boxes.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(lx, hx, ly, hy), b| (lx.min(b.0), hx.max(b.1), ly.min(b.2), hy.max(b.3)),
    )
}

/// `(cell size, cells along x, cells along y)`: a **cubic** cell, so a diagonal's cost
/// does not depend on which diagonal it is — the reference's reason for cubic cells
/// (`routed.py::_grid`). The longer of the two spans sets the cell, and each axis is then
/// covered at that one size, plus the margin on both sides.
fn axes(boxes: &[(f64, f64, f64, f64)], params: &GridParams) -> (f64, u32, u32) {
    let (lx, hx, ly, hy) = bounds(boxes);
    let span = (hx - lx).max(hy - ly).max(f64::MIN_POSITIVE);
    let cell = span / f64::from(params.resolution);
    let count = |from: f64, to: f64| {
        let cover = ((to - from) / cell).ceil();
        let n = if cover.is_finite() && cover > 0.0 {
            cover as u32
        } else {
            0
        };
        n.saturating_add(2 * params.margin).max(1)
    };
    (cell, count(lx, hx), count(ly, hy))
}

/// Every node's footprint box `(x0, x1, y0, y1)`: a point's own position, a circle's
/// bounding square, a box's rectangle. One rule for all three geometry kinds, so a route
/// composes with every layout rather than only with the point ones.
pub fn footprints(geometry: &NodeGeometry) -> Vec<(f64, f64, f64, f64)> {
    let at = |c: &[f32], i: usize| f64::from(c[i]);
    match geometry {
        NodeGeometry::Point { x, y } => (0..x.len())
            .map(|i| (at(x, i), at(x, i), at(y, i), at(y, i)))
            .collect(),
        NodeGeometry::Circle { x, y, r } => (0..x.len())
            .map(|i| {
                let (cx, cy) = (at(x, i), at(y, i));
                (cx - at(r, i), cx + at(r, i), cy - at(r, i), cy + at(r, i))
            })
            .collect(),
        NodeGeometry::Box { x, y, w, h } => (0..x.len())
            .map(|i| {
                let (cx, cy) = (at(x, i), at(y, i));
                (
                    cx - at(w, i) / 2.0,
                    cx + at(w, i) / 2.0,
                    cy - at(h, i) / 2.0,
                    cy + at(h, i) / 2.0,
                )
            })
            .collect(),
    }
}

/// `Err(NonFinite)` naming the first column that holds a NaN or an infinity. Columns are
/// visited in wire order, so the refusal does not depend on any iteration order (D9).
fn check_finite(geometry: &NodeGeometry) -> Result<(), StageError> {
    for (name, column) in geometry.columns() {
        if column.iter().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite {
                column: match name {
                    "x" => "node.x",
                    "y" => "node.y",
                    "r" => "node.r",
                    "w" => "node.w",
                    _ => "node.h",
                },
            });
        }
    }
    Ok(())
}

/// The cell index holding `value` on one axis, by the module's boundary rule: cells are
/// `[origin + k·cell, origin + (k+1)·cell)`, so an exact boundary belongs to the cell it
/// opens, and a quotient within [`BOUNDARY_TOL`] of a whole number is that whole number.
/// Clamped into `0..n`.
pub fn axis_cell(value: f64, origin: f64, cell: f64, n: u32) -> u32 {
    let q = (value - origin) / cell;
    let nearest = libm::round(q);
    let snapped = if libm::fabs(q - nearest) <= BOUNDARY_TOL {
        nearest
    } else {
        libm::floor(q)
    };
    snapped.clamp(0.0, f64::from(n) - 1.0) as u32
}
