//! The frozen force set (`P56_SPEC.md` decision 7, devil C10): link, many-body (Barnes-Hut),
//! center, collide — cluster forces excluded (no node groups before Phase 10, a deviation
//! recorded here and in `docs/measurements/phase06-stress.md`). Every constant below is the
//! production engine's own value, not d3's stock default, cited to its source line so a
//! later change is a diff against a real number rather than a guess.
//!
//! Sources, cited once, used by [`barnes_hut`](super::barnes_hut):
//! - `chargeStrength: -90`, `chargeDistanceMax: 520`, `linkDistance: 60`,
//!   `linkStrengthScale: 0.15`, `collideRadius: 16`, `centerStrength: 1`,
//!   `velocityDecay: 0.42` — `osionos/packages/graph-engine/src/core/layout/params.ts:28-34`
//!   (`DEFAULT_LAYOUT_PARAMS`).
//! - Per-link distance/strength formulas —
//!   `osionos/.../forceLayout.ts:206-207`: `distance = linkDistance / max(0.4, l.strength)`,
//!   `strength = min(0.7, linkStrengthScale * l.strength)`.
//! - `theta = 0.9` — this is **d3's own stock default** (`d3-force/src/manyBody.js:15`,
//!   `theta2 = 0.81`, so `theta = sqrt(0.81) = 0.9`); the engine never overrides it, so
//!   there is nothing to cite beyond d3 itself.
//! - `alphaDecay = 0.06` — `osionos/.../forceLayout.ts:150`. `alphaMin = 0.001` and
//!   `distanceMin (many-body) = 1` are d3's stock defaults (`d3-force/src/simulation.js:19`,
//!   `manyBody.js:13`), not overridden by the engine.
//! - `TICKS = 112` — `prompt.md` §5.2: `0.94^k < 0.001` at `alphaDecay = 0.06` first holds
//!   at `k = 112`.

/// One fixed force-layout configuration. No setters: the frozen set is a single point,
/// not a knob surface, for this phase (`Default` is the only value used by the gate).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ForceParams {
    /// Many-body (Barnes-Hut) repulsion strength, the same scalar for every node.
    pub charge_strength: f64,
    /// Barnes-Hut opening angle.
    ///
    /// Ponytail: trades accuracy for speed. Failing input: two dense, well-separated
    /// clusters whose combined bounding box is small relative to their distance from a
    /// far node — the opening-angle test then treats a whole cluster as one point mass
    /// too eagerly. Direction: over-clumping (distant structure visibly collapses
    /// together), not under-reporting. Escape hatch: `theta` is a fixed engine constant
    /// here, not a per-call knob; a future override would trade the speed back for
    /// accuracy (`phase-06-iterative-spectral-mds.md`'s Ponytail requirements).
    pub theta: f64,
    /// Many-body: force is not computed for two points closer than this.
    pub distance_min: f64,
    /// Many-body: force is zeroed beyond this distance (a performance cutoff).
    pub distance_max: f64,
    /// Base link distance; a link's own is this over `max(0.4, edge.strength)`.
    pub link_distance: f64,
    /// Link strength scale; a link's own is `min(0.7, scale * edge.strength)`.
    pub link_strength_scale: f64,
    /// Collision radius, the same for every node.
    pub collide_radius: f64,
    /// Center-force pull strength.
    pub center_strength: f64,
    /// Per-tick velocity multiplier (`1 - d3's velocityDecay` in d3's own terms).
    pub velocity_decay: f64,
    /// `alpha` geometric decay per tick.
    pub alpha_decay: f64,
    /// Below this `alpha` the simulation is considered settled.
    pub alpha_min: f64,
    /// Starting `alpha`.
    pub initial_alpha: f64,
}

impl Default for ForceParams {
    fn default() -> Self {
        Self {
            charge_strength: -90.0,
            theta: 0.9,
            distance_min: 1.0,
            distance_max: 520.0,
            link_distance: 60.0,
            link_strength_scale: 0.15,
            collide_radius: 16.0,
            center_strength: 1.0,
            velocity_decay: 1.0 - 0.42,
            alpha_decay: 0.06,
            alpha_min: 0.001,
            initial_alpha: 1.0,
        }
    }
}

/// Ticks to settle at the frozen `alpha_decay` (`prompt.md` §5.2): the least `k` with
/// `(1 - alpha_decay)^k < alpha_min`.
pub const TICKS: u32 = 112;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_engines_cited_values() {
        let p = ForceParams::default();
        assert_eq!(p.charge_strength, -90.0);
        assert_eq!(p.theta, 0.9);
        assert_eq!(p.distance_max, 520.0);
        assert_eq!(p.link_distance, 60.0);
        assert_eq!(p.link_strength_scale, 0.15);
        assert_eq!(p.collide_radius, 16.0);
        assert_eq!(p.center_strength, 1.0);
        assert!((p.velocity_decay - 0.58).abs() < 1e-12);
        assert_eq!(p.alpha_decay, 0.06);
    }

    #[test]
    fn ticks_is_the_least_k_below_alpha_min() {
        let p = ForceParams::default();
        let decay = 1.0 - p.alpha_decay;
        let at = |k: u32| {
            let mut a = 1.0f64;
            for _ in 0..k {
                a *= decay;
            }
            a
        };
        assert!(at(TICKS) < p.alpha_min);
        assert!(at(TICKS - 1) >= p.alpha_min);
    }
}
