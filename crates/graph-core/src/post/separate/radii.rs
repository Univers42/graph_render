//! Every node's collision radius in layout units, and the largest of them.
//!
//! One number per node, whatever geometry kind the layout emitted — the three kinds differ in
//! the columns they add, never in the two they share, so a pass that works on radii works on
//! all of them without branching per neighbour.
//!
//! | kind | radius | where it comes from |
//! | --- | --- | --- |
//! | `Circle` | `r` | the layout's own column |
//! | `Box` | the circumscribed radius `√((w/2)² + (h/2)²)` | the layout's own columns |
//! | `Point` | `point_radius` | **the caller**, default `0.0` |
//!
//! The `Box` row is the circumscribed circle, not the inscribed one, and that is a decision
//! with a consequence: two boxes can be disjoint while their bounding discs overlap, so this
//! **over-separates a `Box` layout** and never under-separates one. The safe direction for an
//! invariant, the wrong direction for fidelity to the reference — which also works in circles
//! (Graphviz's `getSizes` returns one radius per node). Ponytail marker; also on [`super::META`].

use crate::stage::StageError;
use graph_contract::geometry::NodeGeometry;

/// One radius per node, in node order.
pub fn of(nodes: &NodeGeometry, point_radius: f64) -> Result<Vec<f32>, StageError> {
    match nodes {
        NodeGeometry::Point { x, .. } => Ok(vec![point_radius as f32; x.len()]),
        NodeGeometry::Circle { r, .. } => {
            check_radii(r)?;
            Ok(r.clone())
        }
        NodeGeometry::Box { x, w, h, .. } => {
            check_radii(w)?;
            check_radii(h)?;
            Ok((0..x.len()).map(|i| half_diagonal(w[i], h[i])).collect())
        }
    }
}

/// The largest of `radii`, or 0 for an empty layout — which is not an error: a graph with no
/// nodes has no discs to separate.
pub fn largest(radii: &[f32]) -> f32 {
    radii.iter().copied().fold(0.0f32, f32::max)
}

/// A box's circumscribed radius: the distance from its centre to a corner.
///
/// `libm::sqrt` (D1) and no `powi`, and deliberately **not** `hypot` — `hypot` is the better
/// answer for `w² + h²` but this is one `sqrt` of a sum of two squares and the overflow guard
/// `hypot` buys is worth nothing at layout-unit magnitudes, where `w` is a drawing size.
fn half_diagonal(w: f32, h: f32) -> f32 {
    let (hw, hh) = (w * 0.5, h * 0.5);
    libm::sqrtf(hw * hw + hh * hh)
}

/// A size column that is not finite or is negative would make the required separation NaN or
/// negative, and a negative separation means every pair "overlaps" — the pass would then
/// scatter the drawing rather than refusing the input. One `f32::max` chain over the column,
/// in index order (D2).
fn check_radii(column: &[f32]) -> Result<(), StageError> {
    if column.iter().all(|v| v.is_finite() && *v >= 0.0) {
        return Ok(());
    }
    Err(StageError::Param {
        name: "node size",
        rule: "finite and not negative",
    })
}