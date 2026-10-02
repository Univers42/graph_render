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

/// The smallest span an axis is measured over, the reference's floor (`routed.py:55`,
/// `np.maximum(max - lo, 1e-9)`), applied per axis before the cell is sized: a flat axis
/// is one cell wide plus the margin, as in the reference.
///
/// Ponytail: an absolute floor in layout units. Failing input: a layout narrower than 1e-9
/// on its longer axis is measured as 1e-9 wide, so it fills only part of the grid and
/// nodes the resolution would separate can share a cell. Direction: those edges draw
/// straight and are counted as fallbacks, never routed wrongly. Escape hatch: rescale.
const MIN_SPAN: f64 = 1e-9;

/// The most cells a grid may hold, `u32::MAX / 8`: the routing CSR keeps eight entries
/// per cell and names an edge `source · 8 + slot`, and both must fit u32.
///
/// Ponytail: a width limit, not a memory one. Failing input: a grid near the ceiling
/// (resolution about 23 000 on a square layout) is accepted, and routing it builds
/// 8 · cells `(u32, u32)` pairs, about 34 GB (arithmetic, not measured), which aborts on
/// allocation instead of returning an error. Direction: over-accepts; no grid past the
/// ceiling is ever indexed. Escape hatch: a lower `resolution`.
const MAX_CELLS: u64 = (u32::MAX / 8) as u64;

/// `(cell size, cells along x, cells along y)`: a **cubic** cell, so a diagonal's cost
/// does not depend on which diagonal it is — the reference's reason for cubic cells
/// (`routed.py::_grid`). The longer of the two spans sets the cell, and each axis is then
/// covered at that one size, plus the margin on both sides. `Err(Param)` past
/// [`MAX_CELLS`], counted in u64 so neither the margin nor the product can wrap.
pub(super) fn axes(
    boxes: &[(f64, f64, f64, f64)],
    params: &GridParams,
) -> Result<(f64, u32, u32), StageError> {
    let (lx, hx, ly, hy) = bounds(boxes);
    let (sx, sy) = ((hx - lx).max(MIN_SPAN), (hy - ly).max(MIN_SPAN));
    let cell = sx.max(sy) / f64::from(params.resolution);
    let margin = 2 * u64::from(params.margin);
    let count = |span: f64| (span / cell).ceil().max(1.0) as u64 + margin;
    let (nx, ny) = (count(sx), count(sy));
    let fits = nx.checked_mul(ny).is_some_and(|cells| cells <= MAX_CELLS);
    match (fits, u32::try_from(nx), u32::try_from(ny)) {
        (true, Ok(nx), Ok(ny)) => Ok((cell, nx, ny)),
        _ => Err(StageError::Param {
            name: "resolution",
            rule: "with the margin, at most u32::MAX / 8 cells",
        }),
    }
}

/// `Err(Param)` for a zero `resolution`, or a `clearance` that is negative or not finite:
/// a negative pad shrinks a footprint past itself, and an inverted box marks no cell, so
/// its node would be silently passable.
pub(super) fn check_params(params: &GridParams) -> Result<(), StageError> {
    if params.resolution == 0 {
        return Err(StageError::Param {
            name: "resolution",
            rule: "at least 1",
        });
    }
    if params.clearance.is_finite() && params.clearance >= 0.0 {
        Ok(())
    } else {
        Err(StageError::Param {
            name: "clearance",
            rule: "finite and at least 0",
        })
    }
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
///
/// `z` is passed as `None`: the grid index is a 2D measurement over `x`/`y` and the size
/// columns, and a z coordinate has no place in a cell. The snapshot's own z column is
/// checked and carried by `Snapshot::new`, and no pass here rewrites it (verdict
/// condition 6), so a 3D snapshot keeps its z through the grid index untouched.
///
/// Then the snapshot contract's own rule, reused rather than restated: every column as
/// long as `x`, and no negative `r`, `w` or `h` (an inverted footprint marks no cell).
pub(crate) fn check_geometry(geometry: &NodeGeometry) -> Result<(), StageError> {
    for (name, column) in geometry.columns_dim(None) {
        if column.iter().any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite {
                column: match name {
                    "x" => "node.x",
                    "y" => "node.y",
                    "z" => "node.z",
                    "r" => "node.r",
                    "w" => "node.w",
                    _ => "node.h",
                },
            });
        }
    }
    let n = geometry.columns().first().map_or(0, |(_, x)| x.len());
    let n = u32::try_from(n).unwrap_or(u32::MAX);
    geometry.check(n, None).map_err(StageError::Snapshot)
}
