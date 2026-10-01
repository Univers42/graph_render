//! Shared plumbing of the closed-form point layouts (`random`, `circular::ring`,
//! `bipartite`, `spiral`): networkx's `rescale_layout` and the `Point`/`Line` geometry
//! every one of them emits.
//!
//! Reference: networkx 3.6 `drawing/layout.py` `rescale_layout`. Coordinates are computed
//! in `f64` and cast to `f32` once, at the end; networkx keeps some intermediates in
//! `f32` (the ring's angles), so the differential against it is a tolerance, not bytes.
//!
//! **The two halves are separate functions on purpose.** [`merge`] is the reduction a
//! threaded layout must not let a runner divide, and [`recentre`] is the pass that cannot
//! usefully be split at all; a layout that hands its per-node gather to a runner fills a
//! column and comes here for both, and a layout that does not (the tree ones) comes here
//! for the same two, so there is one spelling of the arithmetic either way.

use super::Geometry;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// networkx `rescale_layout(pos, scale=1)`: subtract the centroid, then divide by the
/// largest absolute coordinate. A cloud that collapses to a point is left at the origin.
/// The centroid is summed in index order, so the result is fixed on every target.
///
/// The serial tier, and exactly [`rescale_under`] with the control off — not a second copy
/// of the arithmetic, so a layout that does not thread yet cannot drift from one that does.
pub(super) fn rescale(x: &mut [f64], y: &mut [f64]) {
    rescale_under(x, y, false)
}

/// [`rescale`] with the merge's negative control reachable, so a host can run a
/// *deliberately wrong* tier and the gate must go red.
///
/// `split` is [`coords`]' own slice of that control, exactly the shape
/// `barnes_hut::charge`'s is: it makes the centroid merge read the **next** node's term
/// into this node's, which is the shape a wrong partition of the outputs takes. It is a
/// parameter rather than a `cfg` or an environment read for the same reason that one is —
/// graph-core reads no clock, no environment and no hardware, and the host supplies even
/// the mutation.
pub(super) fn rescale_under(x: &mut [f64], y: &mut [f64], split: bool) {
    if x.is_empty() {
        return;
    }
    let (mean_x, mean_y) = centroid(x, y, split);
    recentre(x, y, mean_x, mean_y);
}

/// The centroid of the cloud, from the two ordered merges below.
fn centroid(x: &[f64], y: &[f64], split: bool) -> (f64, f64) {
    let count = x.len() as f64;
    (merge(x, split) / count, merge(y, split) / count)
}

/// The merge is a straight loop over `column` in ascending node index, which is the one
/// place the division could go wrong and the reason it is written as a loop rather than
/// left to the runner: each node adds its *own* coordinate to its *own* sum, and the
/// honest path cannot pick up a neighbour's term no matter how the gather was sliced.
///
/// That is also why the control has to **steal** one to move anything, and why the honest
/// arm is term for term what `Iterator::sum` over `f64` does — `0.0` and then every value
/// in order, which is what a `fold` from zero is, so a threaded run and a serial one sum
/// the same partials in the same order and land on the same bits.
fn merge(column: &[f64], split: bool) -> f64 {
    let mut total = 0.0_f64;
    for (i, &value) in column.iter().enumerate() {
        total += if split {
            value + column.get(i + 1).copied().unwrap_or(0.0)
        } else {
            value
        };
    }
    total
}

/// Subtract the centroid, then divide by the largest absolute coordinate.
///
/// **One pass, and serial, deliberately.** It is an `f64` max fold and a divide over the
/// whole cloud: splitting it would split nothing worth splitting, and the fold's order is
/// part of the bytes as much as the sum's is.
fn recentre(x: &mut [f64], y: &mut [f64], mean_x: f64, mean_y: f64) {
    let mut limit = 0.0_f64;
    for (px, py) in x.iter_mut().zip(y.iter_mut()) {
        *px -= mean_x;
        *py -= mean_y;
        limit = limit.max(px.abs()).max(py.abs());
    }
    if limit > 0.0 {
        for value in x.iter_mut().chain(y.iter_mut()) {
            *value /= limit;
        }
    }
}

/// `Point` nodes at `(x, y)` narrowed to `f32`, straight `Line` edges, no notes.
pub(super) fn point_geometry(x: &[f64], y: &[f64]) -> Geometry {
    point_geometry_with(x, y, None)
}

/// `Point` nodes at `(x, y)` with an optional `z`, narrowed to `f32` in one place.
///
/// The one constructor both arities go through, so a 3D arm cannot narrow its z column
/// differently from its x and y (`in_space` is what labels the snapshot 3D; the narrowing
/// is what puts the bytes in).
pub(super) fn point_geometry_with(x: &[f64], y: &[f64], z: Option<&[f64]>) -> Geometry {
    let narrow = |column: &[f64]| column.iter().map(|&v| v as f32).collect::<Vec<f32>>();
    let nodes = NodeGeometry::Point {
        x: narrow(x),
        y: narrow(y),
    };
    match z {
        Some(column) => Geometry::in_space(nodes, EdgeGeometry::Line, Vec::new(), narrow(column)),
        None => Geometry::planar(nodes, EdgeGeometry::Line, Vec::new()),
    }
}

#[cfg(test)]
mod tests;

/// Test helpers the closed-form layouts' tests share.
#[cfg(test)]
pub(super) mod probe;
