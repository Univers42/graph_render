//! `_igraph_fit_positions` (`SciGraphs/core/scigraphs_core/mesh/layouts/igraph_layouts.py:24-42`),
//! the one post-step every igraph helper in that file ends with.
//!
//! **This is the reference's own convention and it lives here, in the arm, never in a motor
//! layout.** The same convention SciGraphs applies to Graphviz output lives in this crate's
//! `conformance/motor.rs` for the same reason (`CLAUDE.md`, the Graphviz-ports constraint): a
//! port of a igraph layout is igraph's algorithm and nothing else, and the fitting the caller
//! does afterwards is the caller's business. Moving this into `graph-core` would put a SciGraphs
//! convention inside a layout whose oracle is the C library, and the layout's own `scale`
//! parameter would then mean two things.
//!
//! The arithmetic is line for line: subtract the per-axis mean, divide by the largest magnitude
//! over **all three** axes, multiply by `scale`. One departure, stated: `positions.mean(axis=0)`
//! is numpy's pairwise summation and this is a fixed ascending-index sum (D3), so the two can
//! differ in the last ulp of the mean — a uniform translation, which the Procrustes alignment
//! the matrix reports is invariant to.

/// The motor ids whose SciGraphs reference helper ends in `_igraph_fit_positions`.
///
/// **Four of the six igraph rows, not six.** `igraph_layouts.py` ends every igraph helper with the
/// same fit, `IGRAPH_DH` and `IGRAPH_GRAPHOPT` included, but this job owns five rows and the
/// other two are not re-pinned here; listing them would move bytes this change has no measurement
/// for. Named by motor id, not by row name, because `run` is handed an id — and because no id
/// outside this list is used by another row, so the list cannot over-reach.
///
/// `layout.force.fruchterman_reingold_3d` and `layout.force.kamada_kawai_3d` are the ids
/// `IGRAPH_FR` and `IGRAPH_KK` name, because SciGraphs calls both at `dim=3`
/// (`igraph_layouts.py:74`, `:99`) and those are the only motor layouts that answer in three
/// dimensions. The 2D siblings stay registered and stay pinned byte for byte; they are simply
/// not what this reference runs.
pub(super) const FITTED: [&str; 4] = [
    "layout.force.fruchterman_reingold_3d",
    "layout.force.kamada_kawai_3d",
    "layout.force.drl",
    "layout.force.lgl",
];

/// Centres every axis on its own mean and scales the whole drawing so the largest magnitude over
/// all three axes is exactly `scale` (`igraph_layouts.py:34-41`).
///
/// **All three axes share one factor.** `extent` is `np.abs(positions).max()` over the whole
/// array, not per axis, so a `z` of 7 caps a drawing whose `x` spans 9. Computing it that way is
/// the difference between a uniform fit and a per-axis squash, and the reference is uniform.
///
/// A drawing whose extent is 0 — every coordinate equal — is returned untouched rather than
/// divided by zero; `extent > 0` is the reference's own guard (`igraph_layouts.py:39`).
#[cfg(test)]
#[path = "fit/tests.rs"]
mod tests;

pub(super) fn fit(points: &mut [[f64; 3]], scale: f64) {
    if points.is_empty() {
        return;
    }
    let mut mean = [0.0_f64; 3];
    for axis in 0..3 {
        let mut sum = 0.0;
        for point in points.iter() {
            sum += point[axis];
        }
        mean[axis] = sum / points.len() as f64;
    }
    let mut extent = 0.0_f64;
    for point in points.iter_mut() {
        for axis in 0..3 {
            point[axis] -= mean[axis];
            extent = extent.max(point[axis].abs());
        }
    }
    if extent > 0.0 {
        let factor = scale / extent;
        for point in points.iter_mut() {
            for axis in 0..3 {
                point[axis] *= factor;
            }
        }
    }
}
