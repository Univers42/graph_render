//! `post.bundle.fdeb` — force-directed edge bundling, Holten & van Wijk 2009: every edge
//! becomes a polyline of subdivision points, and each point is pulled toward the
//! corresponding points of the edges it is *compatible* with while a spring holds it
//! against the two points beside it.
//!
//! **The reference.** Ported from `SciGraphs/engine/scigraphs_engine/bundling/fdeb.py` (the
//! schedule, the four-term compatibility, the threshold prune, the arc-length resample, the
//! pinned endpoints) with the attraction of the CPU implementation in
//! `SciGraphs/core/scigraphs_core/mesh/edge_styles.py:344-416`, which is the same algorithm
//! without the GPU grid around it. Every constant is the reference's own:
//!
//! | constant | value | where |
//! |---|---|---|
//! | `STEP0` | 0.6 | `fdeb.py:30`, the first cycle's step, halved per cycle |
//! | `SPRING_GAIN` | 0.5 | `fdeb.py:33`, `edge_styles.py:388` |
//! | `SOFTEN_FRAC` | 0.01 | `fdeb.py:36`, softening the attraction's singularity |
//! | `RADIUS_FRAC` | 0.05 | `fdeb.py:39` `DEFAULT_RADIUS`, a fraction of the diagonal |
//! | `cycles` | 6 | `fdeb.py:38` `DEFAULT_CYCLES` |
//! | `segments` | 12 | the `BUNDLED_DENSE` preset, `edge_styles.py:41` |
//! | `iterations` | 8 | the `BUNDLED_DENSE` preset, `edge_styles.py:43` |
//! | `strength` | 0.8 | the `BUNDLED_DENSE` preset, `edge_styles.py:42` |
//! | `threshold` | 0.6 | `bundle_threshold`, read at `fdeb.py:259`; default `edge_styles.py:348` |
//! | visibility | on | `fdeb.py:40` `DEFAULT_VISIBILITY` |
//!
//! **Gather form (D10).** One iteration is a double buffer: subdivision point `(e, p)`
//! reads the start-of-step state of its own two neighbours and of its partners'
//! corresponding points, and writes only itself. Nothing scatters into another point's
//! accumulator, so a threaded tier can partition points by edge and a SIMD tier can
//! vectorise across them without changing a byte. Nothing is parallel *inside* one point's
//! sum (D3): the partners are walked in ascending edge index, always.
//!
//! **Determinism.** The subdivision schedule is a function of the parameters alone, never of
//! the data; the pair list is sorted by `(edge index, edge index)`; every sum is taken over
//! that list in that order; there is no `mul_add` (D2); every length and distance is
//! `libm`'s, and `sqrt`/`hypot` are the only transcendentals at all. The compatibility of a
//! pair depends only on the two edges' endpoints, which never move, so the pair list is
//! built once and read by every iteration of every cycle — exact, not an approximation, and
//! the reason the list is affordable at all.
//!
//! **Pruning.** The reference truncates interactions at a radius with a uniform grid
//! (`fdeb.py:_pairs_grid`) and then drops every pair below the compatibility threshold.
//! Only the threshold is ported: the grid belongs to the routing slice's spatial index, and
//! Phase 8 adds it where a measurement demands it, not here. The cost is therefore `O(m²)`
//! to build the pair list, then, per iteration, every subdivision point walking its edge's
//! surviving row — `O(k · R)` for `R` surviving pairs, up to `m²/2` when nothing is pruned.
//! [`META`]'s `scale_ceiling` is measured over both, and its `complexity` row states both.
//!
//! **Ponytail (threshold).** The compatibility threshold is a knob, not a computation: a
//! pair scoring below it never attracts. The failing input is a pair of edges just under
//! the threshold that a viewer would expect to see bundled. Direction: **under-bundling,
//! cosmetic** — every edge is still drawn, on its own path, and no endpoint ever moves.
//! Escape hatch: [`FdebParams::threshold`], down to the paper's own 0.05.
//!
//! **Ponytail (`scale_ceiling`).** One timed run per size on one host, release, with no
//! warm-up and no repeat, so it under-reports on a loaded host and drifts by tens of
//! percent between runs. The ceiling is set with that margin in hand, not at the largest
//! size that squeaked in. Re-measure with `graph-cli ink --nodes N`; the sweep behind the
//! number is written up in `docs/measurements/phase08-ink.md`.

mod compat;
mod fixture;
mod pairs;
mod points;
#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::post::{Bundled, Metadata, centres};
use crate::stage::StageError;
use compat::Frames;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind};
use pairs::PairList;
use points::Points;

pub use fixture::{FIXTURES, hairball, load};

/// The first cycle's step, halved every cycle after (`fdeb.py:30`).
pub const STEP0: f32 = 0.6;
/// The spring term's gain, matching the vectorized FDEB in `edge_styles.py` (`fdeb.py:33`).
pub const SPRING_GAIN: f32 = 0.5;
/// The attraction is evaluated where bundled points converge, so its weight divides by
/// `d² + (SOFTEN_FRAC · radius)²` (`fdeb.py:36`).
pub const SOFTEN_FRAC: f32 = 0.01;
/// The reference's interaction radius as a fraction of the drawing's diagonal
/// (`fdeb.py:39`). It no longer truncates interactions — the spatial index is not ported
/// here — and survives only as the scale the softening is taken against, so the attraction
/// is dimensionless.
pub const RADIUS_FRAC: f32 = 0.05;

/// The post capability's id, which is also its ledger row's.
pub const ID: &str = "post.bundle.fdeb";

/// Edge count past which FDEB stops being usable, and why it is this one.
///
/// Measured, not estimated: `graph-cli ink --nodes N --layout circular.radial` reports the
/// wall time of one pass natively in `ge-rust` (x86_64, release). The sweep that produced
/// this number is in `docs/measurements/phase08-ink.md`; re-run the command to refresh it.
/// FDEB is a one-shot pass, so it is held to a second — the budget
/// `layout.packing.circle`'s ceiling is held to — and not to the 16.67 ms frame budget,
/// which bounds a per-frame tick rather than a redraw.
pub const FDEB_CEILING: u64 = 6_900;

/// FDEB's ledger row.
pub const META: Metadata = Metadata {
    tier: 1,
    edges: EdgeGeometryKind::Polyline,
    oracle: "hand: Holten & van Wijk 2009, ported from SciGraphs \
engine/scigraphs_engine/bundling/fdeb.py (the schedule, the four-term compatibility, the threshold \
prune, the arc-length resample) with the CPU attraction of \
core/scigraphs_core/mesh/edge_styles.py:344-416. Every constant is named in this module's doc and \
pinned by a unit test; no third-party bundler is a byte-for-byte oracle, because the reference is \
Python over 3D numpy and this is 2D f32",
    complexity: "O(m^2) to build the pair list once, then O(k x R) per iteration: each of an edge's k \
subdivision points (k <= MAX_SEGMENTS + 2) walks its edge's surviving row, R <= m(m-1)/2 pairs in all. \
Worst case O(m^2 x k x I), I the schedule's total iterations; the threshold prune is what keeps R \
below m^2 in practice, and Bundled::pairs reports it",
    scale_ceiling: FDEB_CEILING,
    degradation: "past the ceiling the pass still returns finite geometry and never refuses — it stops \
fitting a one-second budget, at O(m^2) on the pair list. There is no built-in cutoff, so a caller \
inside a frame budget applies its own; Bundled::pairs is the count it can read to price the pass \
before paying for it",
    ponytail: "Ponytail (threshold): edges whose compatibility with every other edge is below the \
threshold (0.6 by default) never attract and are drawn unbundled, and Bundled::unbundled counts them. \
Failing input: a pair just under the threshold that a viewer expects bundled. Direction: \
under-bundling, cosmetic — every edge is still drawn and no endpoint moves. Escape hatch: \
FdebParams::threshold, down to the paper's 0.05. Ponytail (scale_ceiling): one timed run per size on \
one host, no warm-up, release build (docs/measurements/phase08-ink.md); the spatial index that would \
cut the O(m^2) pair list is not ported here. Ponytail (ink): the raster is 128 cells per axis, a \
stated resolution rather than a truth — see post/ink.rs",
};

/// FDEB's parameters. `Default` is the reference's own configuration, so a hashed snapshot
/// is pinned to it and no caller has to restate it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FdebParams {
    /// Subdivision cycles (`fdeb_cycles`, `fdeb.py:46`). Floored at 1; above [`MAX_CYCLES`]
    /// refused.
    pub cycles: u32,
    /// Iterations in the first cycle (`bundle_iterations`, `edge_styles.py:43`). Each later
    /// cycle takes two thirds of the one before, rounded up, and never fewer than 1.
    pub iterations: u32,
    /// Cap on subdivision points per edge (`segments`, `edge_styles.py:41`): the schedule
    /// doubles them per cycle and stops here. Floored at 1; above [`MAX_SEGMENTS`] refused.
    pub segments: u32,
    /// The attraction's gain (`bundle_strength`, `edge_styles.py:42`), in `0..=1`. The
    /// reference clips it into range silently; this refuses it, because a caller who passed
    /// 4 meant something other than 4.
    pub strength: f32,
    /// The compatibility threshold (`bundle_threshold`, `fdeb.py:259`): a pair
    /// scoring below it never attracts. In `0..=1`.
    pub threshold: f32,
    /// Whether the visibility term `Cv` is applied (`fdeb_visibility`, `fdeb.py:260`).
    pub visibility: bool,
}

impl Default for FdebParams {
    fn default() -> Self {
        Self {
            cycles: 6,
            iterations: 8,
            segments: 12,
            strength: 0.8,
            threshold: 0.6,
            visibility: true,
        }
    }
}

/// The schedule: `(subdivision points, step, iterations)` per cycle, exactly
/// `fdeb.py:schedule`. The point count starts at 1 and doubles, capped at `segments`; the
/// step starts at [`STEP0`] and halves; the iteration count falls to two thirds, rounded
/// up, so later cycles only refine. At least one cycle, one iteration and one subdivision
/// point, whatever the parameters say.
pub fn schedule(params: &FdebParams) -> Vec<(u32, f32, u32)> {
    let cap = params.segments.max(1);
    let (mut points, mut step, mut iters) = (1, STEP0, params.iterations.max(1));
    let mut out = Vec::new();
    for _ in 0..params.cycles.max(1) {
        out.push((points.min(cap), step, iters));
        points = points.saturating_mul(2);
        step *= 0.5;
        iters = two_thirds_up(iters);
    }
    out
}

/// `round(i · 2 / 3)` with at least 1, in integers. The reference rounds a `float`; the
/// fraction is never exactly a half (`2i/3 = k + 1/2` has no integer solution), so rounding
/// half away from zero and rounding half to even agree on every `i`, and integer arithmetic
/// removes the question.
fn two_thirds_up(iterations: u32) -> u32 {
    ((iterations.saturating_mul(2) + 1) / 3).max(1)
}

/// FDEB at its default parameters — the run [`ID`] is registered and hashed under.
pub fn run(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    bundle(topology, geometry, &FdebParams::default())
}

/// Bundles `geometry`'s edges over `topology`. Node positions come from the layout and are
/// never moved; the edges' interior points are the layout's own (a `Line`'s none, a
/// `Polyline`'s bends), so the pass composes with every layout rather than assuming a
/// straight input.
pub fn bundle(
    topology: &Topology,
    geometry: &Geometry,
    params: &FdebParams,
) -> Result<Bundled, StageError> {
    check(params)?;
    let (x, y) = centres(&geometry.nodes);
    let edges = topology.edges();
    let frames = Frames::of(x, y, &edges.source, &edges.target)?;
    let list = PairList::of(&frames, params);
    let soften = soften(x, y);
    let schedule = schedule(params);
    let mut points = Points::of(topology, geometry, (x, y), schedule[0].0 + 2);
    for (subdivisions, step, iterations) in schedule {
        points.resample(subdivisions + 2);
        let (rows, per_edge) = points.shape();
        let mut next = Points::empty(rows, per_edge);
        for _ in 0..iterations {
            points.step_into(&mut next, &list, &frames, params.strength, step, soften);
            core::mem::swap(&mut points, &mut next);
        }
    }
    Ok(Bundled {
        // The only way a post pass rebuilds a geometry, so the z column cannot be
        // dropped by forgetting it: it is handed on whatever it was.
        geometry: geometry.with_edges(EdgeGeometry::Polyline(points.interior_paths())),
        pairs: list.total(),
        unbundled: list.unbundled().len() as u32,
    })
}

/// The most subdivision points per edge the pass accepts: the reference panel's own
/// `edge_segments` max (`SciGraphs/properties/edge_style_properties.py:78-85`).
///
/// Ponytail: the reference UI's ceiling, not a measurement. A caller asking for a finer row
/// is refused although the arithmetic would hold well past it; what it buys is that
/// `segments + 2` and the row buffers can never overflow. Escape hatch: raise the const.
pub const MAX_SEGMENTS: u32 = 32;

/// The most schedule cycles the pass accepts: the reference panel's own `edge_fdeb_cycles`
/// max (`edge_style_properties.py:286-297`).
///
/// Ponytail: the reference UI's ceiling, not a measurement. Past ~5 cycles the point count
/// is already capped by [`MAX_SEGMENTS`] and each further cycle halves an already tiny
/// step, so a refused 11th cycle loses only refinement. Escape hatch: raise the const.
pub const MAX_CYCLES: u32 = 10;

/// Refuses a parameter outside what the pass accepts, rather than clipping it.
fn check(params: &FdebParams) -> Result<(), StageError> {
    for (name, value, max, rule) in [
        (
            "segments",
            params.segments,
            MAX_SEGMENTS,
            "at most MAX_SEGMENTS (32)",
        ),
        (
            "cycles",
            params.cycles,
            MAX_CYCLES,
            "at most MAX_CYCLES (10)",
        ),
    ] {
        if value > max {
            return Err(StageError::Param { name, rule });
        }
    }
    for (name, value, rule) in [
        ("strength", params.strength, "finite and in 0..=1"),
        ("threshold", params.threshold, "finite and in 0..=1"),
    ] {
        if !(value.is_finite() && (0.0..=1.0).contains(&value)) {
            return Err(StageError::Param { name, rule });
        }
    }
    Ok(())
}

/// `(SOFTEN_FRAC · radius)²`, with `radius` the reference's fraction of the drawing's
/// diagonal. A degenerate drawing — every node on one point — has no diagonal, and the
/// guard in the iteration's weight division takes over there.
fn soften(x: &[f32], y: &[f32]) -> f32 {
    let radius = RADIUS_FRAC * libm::hypotf(extent(x), extent(y));
    (SOFTEN_FRAC * radius) * (SOFTEN_FRAC * radius)
}

/// The width of a column: its largest minus its smallest, `0` when it is empty.
fn extent(column: &[f32]) -> f32 {
    let mut low = f32::INFINITY;
    let mut high = f32::NEG_INFINITY;
    for value in column {
        low = low.min(*value);
        high = high.max(*value);
    }
    if column.is_empty() { 0.0 } else { high - low }
}
