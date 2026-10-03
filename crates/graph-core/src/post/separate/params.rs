//! The pass's parameters, their defaults, and the refusals — split out of
//! `separate.rs` for the house 300-line limit.

use crate::layout::Geometry;
use crate::stage::StageError;

/// The pass's parameters. Its defaults are conventions, pinned by a snapshot hash.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparateParams {
    /// Extra gap added to every pair's required separation, in layout units.
    ///
    /// 0 by default: the discs touch, which is the invariant. A caller whose nodes are
    /// drawn with a stroke or a label passes the gap it actually draws.
    pub margin: f64,
    /// The iteration cap, at least 1. The sweep stops early once one moves nothing.
    ///
    /// 512 by default, and that number is a measurement rather than a round guess.
    ///
    /// **A full lattice is the binding case, not a random cloud.** Sweeping the cap over three
    /// lattice sizes at `sweep::OVER_RELAXATION` — a `k × k` grid of discs at pitch equal to
    /// the radius, which is the worst case because every node is symmetrically wedged between
    /// eight neighbours and its own displacements partly cancel:
    ///
    /// | nodes | 128 | 256 | 384 | 448 |
    /// | --- | --- | --- | --- | --- |
    /// | 500 | 69 | 0 | 0 | 0 |
    /// | 1 000 | 1 008 | 7 | 0 | 0 |
    /// | 2 000 | 2 917 | 2 065 | 462 | 0 |
    ///
    /// The numbers are `Bundled::unbundled`, the residual pairs. 448 is the first cap that
    /// clears the largest size measured, so 512 is the default with a margin over it. Over the
    /// 12 randomised feasible packings the same sweep converged on 12 of 12 at a median of 16
    /// and a worst of 120 — a lattice is an order of magnitude harder than a cloud, which is
    /// why the cap is set from the lattice and not from the clouds.
    ///
    /// A caller who wants fewer sweeps trades the cap for a residue, which
    /// `Bundled::unbundled` then reports exactly. The cost is linear in this cap, so it is also
    /// the lever on the pass's `scale_ceiling`.
    pub max_iterations: u32,
    /// The over-relaxation factor: how much further than the bare penetration a node moves
    /// in one sweep. See [`super::sweep::OVER_RELAXATION`] for the measurement behind it.
    ///
    /// A **parameter rather than a constant** so the hash gate's negative control has
    /// something real to move — the same reason `layout.force.neato` publishes `Epsilon`.
    /// `0.0` is legal and is the control's value: it freezes every displacement, so a pass
    /// that cannot move a node leaves every overlap in place and the invariant row goes red.
    pub over_relaxation: f32,
    /// The radius a `Point` counts as, in layout units.
    ///
    /// **0 by default, which makes the pass a no-op on a `Point` layout.** A point has no
    /// size in the snapshot, so the pass will not invent one; a renderer that draws a 12 px
    /// dot passes the radius it draws. This is a decision with a consequence and not an
    /// oversight — see `docs/decisions/node-overlap.md` §2.
    pub point_radius: f64,
}

impl Default for SeparateParams {
    fn default() -> Self {
        Self {
            margin: 0.0,
            max_iterations: 512,
            over_relaxation: crate::post::separate::sweep::OVER_RELAXATION,
            point_radius: 0.0,
        }
    }
}

/// Node count past which the pass stops being usable, and why this one.
///
/// Measured, not estimated: `graph-cli overlap --nodes N --layout layout.grid --radius 1.0`
/// reports the wall time of one pass natively in `ge-rust` (x86_64, release), and
/// `docs/measurements/ux-overlap.md` holds the sweep. A pass is a one-shot redraw, so it is held
/// to a second — the budget `post::fdeb`'s ceiling is held to — and not to the 16.67 ms
/// frame budget, which bounds a per-frame tick rather than a redraw.
///
/// | nodes | 1 000 | 2 000 | 10 000 | 100 000 |
/// | --- | --- | --- | --- | --- |
/// | pass ms (median of 3) | 41 | 146 | 1 069 | 14 937 |
///
/// **10 000 is where the second runs out**, and the number straddles the line rather than
/// clearing it: 1.07 s at 10 000 on a loaded host, 734 ms on an idle one, and 15 s at 100 000
/// — a decade more nodes for fourteen times the time. The pass is not *refused* above the
/// ceiling: `Bundled::unbundled` reports exactly what is left. But a caller waiting fifteen
/// seconds for a redraw is not using it as a redraw.
///
/// **What the ceiling does not claim.** It is a cost ceiling, and at its top the default cap is
/// not enough to clear the invariant: 16 294 pairs still overlap at 10 000 with
/// `max_iterations = 512`, which is 0.3% of the 5.06 M it separated. The invariant is exact only
/// to 2 000 nodes, where the exhaustive `O(n^2)` check runs; past that the default leaves a
/// residue and the caller trades wall time for it with `max_iterations`.
pub const SEPARATE_CEILING: u64 = 10_000;
/// Refuses a geometry or a parameter the pass will not guess at.
///
/// **The z refusal is the one that matters**: a geometry carrying a z column is refused rather
/// than half-processed, because this pass moves `x` and `y` and cannot reach `z`
/// (`super`'s module doc). `StageError::Param` rather than a new variant, so the 3D verdict
/// stands unamended and the error surface does not grow for one pass.
pub fn check(geometry: &Geometry, params: &SeparateParams) -> Result<(), StageError> {
    if geometry.z.is_some() {
        return Err(StageError::Param {
            name: "geometry.z",
            rule: "must be absent",
        });
    }
    check_number("margin", params.margin)?;
    check_number("point_radius", params.point_radius)?;
    check_number("over_relaxation", f64::from(params.over_relaxation))?;
    if params.max_iterations == 0 {
        return Err(StageError::Param {
            name: "max_iterations",
            rule: "at least 1",
        });
    }
    Ok(())
}

/// A finite, non-negative parameter. Clamping any of them would silently draw a different
/// picture from the one the caller asked for — and for `over_relaxation` a clamp would hide
/// the one value the negative control needs, `0`.
fn check_number(name: &'static str, value: f64) -> Result<(), StageError> {
    if value.is_finite() && value >= 0.0 {
        return Ok(());
    }
    Err(StageError::Param {
        name,
        rule: "finite and not negative",
    })
}
