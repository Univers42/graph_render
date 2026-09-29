//! How a grid is measured and filled: the node footprints, the cell arithmetic, and the
//! refusals. Split out of `grid_index.rs` for the house's 300-line limit — this is the
//! *construction* half, and the parent is the *query* half.
//!
//! Nothing here is public except what `grid_index` re-exports, and nothing here decides
//! anything a caller can observe on its own: the boundary rule, the cubic cell and the
//! clearance default are all stated in the parent module's doc, which is the place to look.

use super::GridParams;
use crate::stage::StageError;
use graph_contract::geometry::NodeGeometry;

/// The node bounding box `(min_x, max_x, min_y, max_y)`.
pub(super) fn bounds(boxes: &[(f64, f64, f64, f64)]) -> (f64, f64, f64, f64) {
    boxes.iter().fold(
        (f64::MAX, f64::MIN, f64::MAX, f64::MIN),
        |(lx, hx, ly, hy), b| (lx.min(b.0), hx.max(b.1), ly.min(b.2), hy.max(b.3)),
    )
}

/// `(cell size, cells along x, cells along y)`: a **cubic** cell, so a diagonal's cost
/// does not depend on which diagonal it is — the reference's reason for cubic cells
/// (`routed.py::_grid`). The longer of the two spans sets the cell, and each axis is then
/// covered at that one size, plus the margin on both sides.
pub(super) fn axes(boxes: &[(f64, f64, f64, f64)], params: &GridParams) -> (f64, u32, u32) {
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
pub(super) fn check_finite(geometry: &NodeGeometry) -> Result<(), StageError> {
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
