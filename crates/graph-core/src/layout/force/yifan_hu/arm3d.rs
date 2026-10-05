//! `layout.force.yifan_hu.3d`: the multilevel force layout run in **three** dimensions.
//!
//! Split from `yifan_hu.rs` for the house line cap, and because the 3D arm is a layer rather
//! than an edit: the coarsening ([`hierarchy`]), the greedy matching, the per-level tick
//! budget ([`REFINE`]) and the coarse-first order are the 2D arm's own code, called
//! unchanged. Only two things are new — the settle underneath it
//! ([`settle3d`](fn@settle3d)) and the prolongation offset,
//! which has to leave the plane for the same reason the start does.
//!
//! **This is not the `'2Z'` arm, and the difference is the whole point of shipping it.**
//! `run_2z` runs the 2D simulation and *then* derives a z from the graph's structure, so the
//! third axis never steers anything. Here `z` is one more coordinate in the same difference
//! vectors: it is pushed by many-body repulsion, by the springs and by collide, and it
//! pushes back. That is why the arm has its own settle rather than reusing the 2D one — a
//! 2D simulation plus a derived z is a different picture and a different claim.
//!
//! Ponytail: SciGraphs' `_yifan_hu_layout` asks for `dim = 3` and gets sfdp's own 3D solve
//! (`yifan_hu.py:344`), which is a different force model from this port's in every dimension
//! — so as with the 2D arm, no sfdp output was compared and no coordinate oracle is claimed.
//! Direction: cosmetic (positions and scale differ from the reference for the same graph),
//! never silently wrong. Escape hatch: the 2D arm for a planar solve.

use super::{REFINE, hierarchy};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::SimpleGraph;
use crate::layout::force::barnes_hut::{golden_sphere, settle3d};
use crate::layout::force::params::{ForceParams, TICKS};
use crate::stage::StageError;

/// SciGraphs' 3D yifan_hu, as this port's id.
pub const ID_3D: &str = "layout.force.yifan_hu.3d";

/// The 3D arm: the 2D arm's coarsening and tick budget over a three-axis settle.
pub fn run_3d(topology: &Topology, params: &ForceParams) -> Result<Geometry, StageError> {
    let graph = crate::layout::force::simple_graph(topology);
    let n = topology.node_count();
    space_points(multilevel3d(graph, n, *params))
}

/// The levels, each settled under the 3D tick: the coarse solve first, then every
/// refinement from its parent's positions. Sequential in the levels by construction, exactly
/// as the 2D arm is.
fn multilevel3d(fine: SimpleGraph, n: u32, params: ForceParams) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let (levels, maps) = hierarchy(fine, n);
    let (top, top_n) = levels.last().expect("one level").clone();
    let mut pos = settle3d(top, params, golden_sphere(top_n), (TICKS, 1.0));
    for k in (0..maps.len()).rev() {
        let start = prolong3d(&pos, &maps[k], params.link_distance);
        pos = settle3d(levels[k].0.clone(), params, start, REFINE);
    }
    pos
}

/// Each fine node starts at its coarse parent, nudged off it by a point on a small sphere of
/// its own index, so two children of one parent do not coincide and no child starts in the
/// plane its parent's neighbourhood lies in.
fn prolong3d(
    coarse: &(Vec<f64>, Vec<f64>, Vec<f64>),
    map: &[u32],
    link_distance: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let radius = 0.25 * link_distance;
    let golden = core::f64::consts::PI * (3.0 - f64::sqrt(5.0));
    let mut out = (Vec::new(), Vec::new(), Vec::new());
    for (i, &p) in map.iter().enumerate() {
        let (ux, uy, uz) = sphere_offset(i, map.len(), radius, golden);
        out.0.push(coarse.0[p as usize] + ux);
        out.1.push(coarse.1[p as usize] + uy);
        out.2.push(coarse.2[p as usize] + uz);
    }
    out
}

/// The unit-sphere point for row `i` of `n`, scaled by `radius`: the same Fibonacci sphere
/// the start uses, sampled at the level's own row count this time because a level knows its
/// own `n`.
fn sphere_offset(i: usize, n: usize, radius: f64, golden: f64) -> (f64, f64, f64) {
    let y = 1.0 - 2.0 * (i as f64 + 0.5) / n as f64;
    let r = f64::sqrt((1.0 - y * y).max(0.0));
    let angle = golden * i as f64;
    (
        radius * r * libm::cos(angle),
        radius * r * libm::sin(angle),
        radius * y,
    )
}

/// The stage's own three columns, refused if any coordinate is not finite.
///
/// The check covers `z` as well as `x` and `y`: a 3D arm that validated only its in-plane
/// columns would ship a `NaN` z as a finite-looking picture.
fn space_points((x, y, z): (Vec<f64>, Vec<f64>, Vec<f64>)) -> Result<Geometry, StageError> {
    if x.iter().chain(&y).chain(&z).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let column = |a: &[f64]| a.iter().map(|&v| v as f32).collect();
    Ok(Geometry::points(3, column(&x), column(&y), column(&z)))
}
