//! Circle packing (Collins–Stephenson), `layout.packing.circle`: `Circle` geometry — the
//! case SciGraphs itself could not carry through its own contract (`circle_radius` was
//! smuggled onto the mesh object out of band, `circle_packing.py:15-36`). A port of
//! SciGraphs' `_circle_packing_layout` (`circle_packing.py:281-393`) over the planar
//! embedding [`planarity`] certifies, plus its own non-planar fallback (`:407-542`,
//! [`fallback`]).
//!
//! Unlike the other three Phase 3 layouts, circle packing is not a hierarchy layout: it
//! runs over the topology's own edges (every kind, self-loops and multi-edges reduced
//! away once, in [`simple_pairs`], exactly as SciGraphs' own `simple` reduction does, so
//! that both paths below read the same graph),
//! not the repaired hierarchy tree [`crate::layout::hierarchy::Hierarchy`] builds. It
//! therefore carries no note codes 1 or 2, and needs neither roots nor children; only
//! code 3, `packing.approximate`, applies (`docs/decisions/planarity-fallback.md`).
//!
//! **Exact path** (`n >= 3`, `[try_exact]`): plane-test the graph
//! ([`planarity::planar_embedding`]), triangulate the certified embedding
//! ([`planarity::triangulate_embedding`]), build every vertex's triangle flower
//! ([`triangles`]), solve for radii whose corners close on their target angle sum
//! ([`radii`]), walk the triangulation's dual to place centres, and relax the walk's
//! rounding drift back toward tangency ([`placement`]). Every graph edge is part of the
//! triangulation (triangulating only adds edges), so every one ends up tangent.
//!
//! **Fallback** (`[fallback]`): whenever the exact path cannot certify a genuine
//! triangulated disk — the graph is not planar, or (defensively) the triangulation itself
//! does not close into one — and whenever the certified packing's own radii do not
//! converge to a tolerance or crowd past a spread SciGraphs itself calls a warning sign
//! (`_MAX_RADIUS_SPREAD = 1e3`, `circle_packing.py:13`). SciGraphs only *logs* this
//! ("Circles may overlap", `:389-391`); this port also flags it with note code 3, per
//! user decision D-N — a log is invisible to a downstream program.
//!
//! Ponytail: **the packing is exact only for planar input.** The failing input is any
//! graph with a K5 or K3,3 minor (or one whose planar embedding cannot be triangulated
//! into a genuine disk, which should not happen for a certified-planar graph but is
//! treated the same defensively). Direction: **overlap, the dangerous one** — the
//! fallback does not guarantee tangency or non-overlap either. Escape hatch: read note
//! code 3 off the snapshot; its absence is the only trustworthy sign the packing is
//! exact. `docs/decisions/planarity-fallback.md` has the full account, including every
//! other SciGraphs fallback condition this port surfaces the same way.
//!
//! Transcendentals are `libm` throughout (`acos`, `sin`, `cos`, `atan2`, `hypot`, `sqrt`),
//! never `mul_add`, matching every other layout in this crate.

mod fallback;
mod geometry;
mod placement;
mod radii;
mod triangles;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::planarity;
use crate::stage::StageError;
use geometry::{Packed, empty, finish, pair, single, to_geometry};
use radii::Solved;
use std::collections::BTreeSet;
use triangles::Flower;

/// The packing's parameters, all of SciGraphs' own defaults
/// (`_circle_packing_layout(G, iterations=500, scale=5.0)`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CirclePackingParams {
    /// Radius-solver sweeps (`x20`, `circle_packing.py:330`) and tangency-refinement
    /// steps on the exact path; the fallback's own relaxation budget. At least `1` is
    /// always used, even when this is `0`.
    pub iterations: u32,
    /// The finished packing's rough diameter: after centring, its extent (furthest
    /// centre plus largest radius) is scaled to `0.45` of this. Must be finite and above
    /// `0`.
    pub scale: f32,
}

impl Default for CirclePackingParams {
    fn default() -> Self {
        Self {
            iterations: 500,
            scale: 5.0,
        }
    }
}

/// Runs circle packing over `topology` at [`CirclePackingParams::default`].
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_with(topology, &CirclePackingParams::default())
}

/// Runs circle packing over `topology`'s own edges (every kind, self-loops and
/// multi-edges reduced away). Refused only when `scale` is not finite and above `0`.
pub fn run_with(topology: &Topology, params: &CirclePackingParams) -> Result<Geometry, StageError> {
    check_scale(params.scale)?;
    let n = topology.node_count();
    let packed = match n {
        0 => empty(),
        1 => single(params.scale),
        2 => pair(params.scale),
        _ => pack(topology, params),
    };
    Ok(to_geometry(packed))
}

fn check_scale(scale: f32) -> Result<(), StageError> {
    if scale.is_finite() && scale > 0.0 {
        Ok(())
    } else {
        Err(StageError::Param {
            name: "scale",
            rule: "finite and above 0",
        })
    }
}

/// `n >= 3`: the exact path, or the fallback when it cannot certify one
/// (`circle_packing.py:308-311`).
fn pack(topology: &Topology, params: &CirclePackingParams) -> Packed {
    let n = topology.node_count();
    let edges = simple_pairs(topology);
    match try_exact(n, &edges, params) {
        Some(packed) => packed,
        None => fallback::pack(n, &edges, params),
    }
}

/// Every topology edge as a dense-index pair, reduced to a simple graph: self-loops
/// dropped, and each undirected pair kept once, in first-occurrence order.
///
/// This is SciGraphs' own `simple` reduction (`circle_packing.py:59-61` — `u != v` into a
/// `nx.Graph`, which cannot hold a parallel edge), applied once here so *both* paths see
/// the same graph. [`planarity::planar_embedding`] would reduce them again on its own, so
/// the exact path's result is unchanged; the fallback needs it here because it reads the
/// edge list directly — an unreduced list gives a duplicated edge two springs instead of
/// one, and a self-loop a zero-length spring that fights the overlap pass.
///
/// The reduction is by first occurrence, not by sorting, so the edge order the relaxation
/// gathers its springs in is the topology's own (D5: a stable order on a total key).
fn simple_pairs(topology: &Topology) -> Vec<(u32, u32)> {
    let e = topology.edges();
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    let mut out = Vec::with_capacity(topology.edge_count() as usize);
    for i in 0..topology.edge_count() as usize {
        let (u, v) = (e.source[i], e.target[i]);
        if u == v || !seen.insert(pair_key(u, v)) {
            continue;
        }
        out.push((u, v));
    }
    out
}

/// An undirected pair as a single total-order key, for membership only — never iterated.
fn pair_key(u: u32, v: u32) -> u64 {
    let (lo, hi) = if u < v { (u, v) } else { (v, u) };
    u64::from(lo) << 32 | u64::from(hi)
}

/// SciGraphs' own crowding threshold past which a certified packing is treated as failed
/// to embed (`_MAX_RADIUS_SPREAD`, `circle_packing.py:13`).
///
/// Ponytail: a packing whose radii span more than this factor is numerically suspect —
/// not provably wrong, but SciGraphs' own author judged it untrustworthy, and this port
/// keeps that judgement rather than second-guessing it. Failing input: any planar graph
/// whose Euclidean packing genuinely needs a wider spread than this (a very unbalanced
/// tree of near-complete subgraphs, say). Direction: a false positive costs an
/// unnecessary fallback (safe, just cosmetic); a false negative would ship an
/// under-converged packing as exact, which this constant exists to prevent. Escape
/// hatch: none needed — the fallback this triggers is itself flagged, note code 3.
const MAX_RADIUS_SPREAD: f64 = 1e3;

/// The Collins–Stephenson exact path (`circle_packing.py:313-370`). `None` when the
/// graph is not planar, or (defensively) when its triangulation does not close into a
/// genuine disk of triangles or the placement walk does not reach every vertex — the
/// same conditions `_planar_triangulation`'s own checks catch (`circle_packing.py:64-65`, `:78-79`, `:88-89`).
fn try_exact(n: u32, edges: &[(u32, u32)], params: &CirclePackingParams) -> Option<Packed> {
    let embedding0 = planarity::planar_embedding(n, edges)?;
    let (embedding, outer) = planarity::triangulate_embedding(&embedding0);
    let flower = triangles::flower(n, &embedding, &outer)?;
    let aims = radii::packing_aims(&flower.at, &flower.boundary_list);
    let all_free: Vec<u32> = (0..n)
        .filter(|&v| !flower.at[v as usize].is_empty())
        .collect();
    let sweeps = params.iterations.max(1).saturating_mul(20);
    let (solved_radii, approximate) = solve_with_fallback(&flower, &aims, &all_free, sweeps);
    let (positions, placed) = placement::lay_out_packing(&flower.triangles, &solved_radii, n);
    if placed.iter().any(|&p| !p) {
        return None; // Ponytail: see module doc — treated as non-planar, defensively.
    }
    let tri_edges = triangles::tri_edges(&flower.triangles);
    let positions = placement::refine_tangency(
        positions,
        &solved_radii,
        &tri_edges,
        params.iterations.max(1),
    );
    Some(finish(positions, solved_radii, params, approximate))
}

/// Solves for radii over every free vertex; if that does not embed
/// ([`is_embedded`]), retries with the boundary pinned (interior vertices only) and
/// reports the result as approximate either way, matching SciGraphs' own retry
/// (`circle_packing.py:325-347`) but surfacing it as note code 3 instead of only a log.
fn solve_with_fallback(
    flower: &Flower,
    aims: &[f64],
    all_free: &[u32],
    sweeps: u32,
) -> (Vec<f64>, bool) {
    let solved = radii::solve_packing_radii(&flower.at, aims, all_free, sweeps);
    if is_embedded(&solved) {
        return (solved.radii, false);
    }
    let interior: Vec<u32> = all_free
        .iter()
        .copied()
        .filter(|&v| !flower.boundary[v as usize])
        .collect();
    let retried = radii::solve_packing_radii(&flower.at, aims, &interior, sweeps);
    (retried.radii, true)
}

/// Whether `solved` can be certified as an embedding: converged, and not crowding past
/// SciGraphs' own spread limit.
///
/// A non-finite value anywhere is **not** embedded. `f64::max`/`f64::min` ignore `NaN`
/// (they return the other operand), so a plain fold reads an all-`NaN` solve as spread
/// `0` — embedded — and the caller would ship a packing with no note code 3 and nothing
/// trustworthy behind it. `max_error` is checked for finiteness for the same reason: it is
/// an error measure, so a non-finite one says nothing about convergence.
fn is_embedded(solved: &Solved) -> bool {
    solved.max_error.is_finite()
        && solved.max_error < radii::TOLERANCE
        && spread(&solved.radii) <= MAX_RADIUS_SPREAD
}

/// `max / min` over `radii`, floored at `1e-300` on the denominator as SciGraphs' own
/// `max(radii.min(), 1e-300)` does (`circle_packing.py:334`). Any non-finite radius reads
/// as an unbounded spread, so [`is_embedded`] refuses it rather than folding past it.
fn spread(radii: &[f64]) -> f64 {
    if radii.iter().any(|r| !r.is_finite()) {
        return f64::INFINITY;
    }
    let max = radii.iter().cloned().fold(f64::MIN, f64::max);
    let min = radii.iter().cloned().fold(f64::MAX, f64::min).max(1e-300);
    max / min
}

#[cfg(test)]
mod tests;
