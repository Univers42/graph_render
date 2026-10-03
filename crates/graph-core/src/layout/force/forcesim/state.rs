//! The simulation state: the `f32` positions, the masses, and the three constants
//! `_renormalize` derives (`simulation.py:363-389`).
//!
//! ```text
//! k   = SIM_SCALE / max(np.cbrt(max(n, 1)), 1.0)                     np.float64
//! mm  = float(mass.mean())                                           f32 reduction, then f64
//! rep = scaling_ratio * (k ** 2 / max(mm ** 3, 1e-9))                np.float64
//! grv = (gravity * 0.1) * (0.38 * 2.0 * SIM_SCALE ** 2
//!                          / (max(k, 1e-9) * max(mm ** 2, 1e-9)))      np.float64
//! ```
//!
//! **Every one of `k`, `rep` and `grv` is `np.float64`, not a Python float**, because
//! `np.cbrt` returns an `np.float64` and `scale / <np.float64>` stays one. That is the
//! whole reason the repulsion coefficient block and gravity are computed in `f64` (see the
//! module doc's dtype trace). `mass.mean()` is numpy's **pairwise** `f32` sum, unlike
//! `pos.mean(axis=0)`'s sequential one — both reductions are in this kernel and they are
//! not the same order.
//!
//! The normalisation exists so that `1.0` means the same thing at any `n` and `scale`; the
//! reference's own docstring is explicit that the equilibrium it lands on is a fit
//! (`simulation.py:367-370`), not a derivation.

use super::reduce::{column_means_f32, pairwise_f32};
use super::{attraction, gravity, integrate::Integrator, pair_force::Direct, rescale, seed};
use crate::layout::force::SimpleGraph;
use crate::layout::force::forcesim::Fa2ForceSimParams;

/// `_FA2_SIM_SCALE` (`forceatlas.py:29`): the scale `ForceSim` is *constructed* with, and
/// so the one `k` and both norms derive from.
///
/// **Not the dispatcher's `scale`.** `scale=5.0` reaches `_fa2_rescale` and nothing else
/// (`forceatlas.py:147`); the simulation's own scale is this constant
/// (`forceatlas.py:130,138`), so the output scale changes how big the picture is and not
/// how it arranges itself.
const SIM_SCALE: f64 = 5.0;

/// `_FA2_GRAVITY_NORM` (`forceatlas.py:27`): the factor the reference folds into `gravity`
/// before handing it to `ForceSim`.
const GRAVITY_NORM: f64 = 0.1;

/// `_GRAVITY_FIT` (`simulation.py:26`), the reference's own fitted monopole correction.
const GRAVITY_FIT: f64 = 0.38;

/// The clamp under each norm's denominator (`simulation.py:380-381,387`).
const MIN_MASS_POWER: f64 = 1e-9;

/// The clamp under `k` in the gravity norm (`simulation.py:387`).
const MIN_K: f64 = 1e-9;

pub(super) struct Sim {
    /// `self.pos`: `f32`, flat `(n, 3)` in C order — gather form (D10), never a `HashMap`.
    pos: Vec<f32>,
    /// `self.mass`: `(self._deg + 1)` in `f32` (`simulation.py:350`). FA2's mass is
    /// `deg + 1`, which is what separates it from FR's unit masses.
    mass: Vec<f32>,
    /// The simple undirected edge list. Orientation-independent: `_attraction`'s two
    /// `bincount` passes swap symmetrically with the edge's direction, so `lo < hi` costs
    /// nothing (the reference's order comes from `G.edges()` and is neither `lo < hi` nor
    /// the fixture's).
    edges: Vec<(u32, u32)>,
    integrator: Integrator,
    /// `self.repulsion`, renormalised. `np.float64` in the reference.
    repulsion: f64,
    /// `self.gravity`, renormalised and already scaled by [`GRAVITY_NORM`].
    gravity: f64,
    /// `DTYPE((0.01 * self.k) ** 2)`: the squared softening, narrowed once here so the
    /// repulsion loop does not re-derive it per pair.
    soften: f32,
}

impl Sim {
    pub(super) fn new(nodes: usize, graph: &SimpleGraph, params: Fa2ForceSimParams) -> Self {
        let edges: Vec<(u32, u32)> = (0..graph.lo.len())
            .map(|e| (graph.lo[e], graph.hi[e]))
            .collect();
        let mut degree = vec![0u32; nodes];
        for &(a, b) in &edges {
            degree[a as usize] += 1;
            degree[b as usize] += 1;
        }
        let mass: Vec<f32> = degree.iter().map(|&d| (d + 1) as f32).collect();
        let (k, repulsion, gravity) = renormalize(&mass, params);
        let softening = 0.01 * k;
        Self {
            pos: seed::start_positions(nodes, params.seed, SIM_SCALE),
            mass,
            edges,
            integrator: Integrator::new(nodes, k, params.jitter_tolerance),
            repulsion,
            gravity,
            soften: (softening * softening) as f32,
        }
    }

    /// `ForceSim.step(iterations)` (`simulation.py:1056-1085`), the loop and its guard.
    ///
    /// `max(1, int(iterations))` is the reference's own floor, so `iterations = 0` still
    /// runs one step; `n < 2` breaks out, because with fewer than two nodes there is no
    /// pair to repel and no swing to measure.
    pub(super) fn step(&mut self, iterations: u32) {
        for _ in 0..iterations.max(1) {
            if self.mass.len() < 2 {
                break;
            }
            let center_was = column_means_f32(&self.pos, self.mass.len());
            let force = self.force_field();
            self.integrator
                .apply(&mut self.pos, &force, &self.mass, center_was);
        }
    }

    /// `_force_field()` (`simulation.py:1000-1017`): repulsion, then attraction, then
    /// gravity, each added into the same `f32` array, in that order.
    fn force_field(&self) -> Vec<f32> {
        let mut force = Direct::new(&self.pos, &self.mass, self.repulsion, self.soften).run();
        attraction::accumulate(&mut force, &self.pos, &self.mass, &self.edges);
        gravity::accumulate(&mut force, &self.pos, &self.mass, self.gravity);
        force
    }

    /// `_fa2_rescale(np.asarray(self.pos, dtype=np.float64), scale)`.
    pub(super) fn rescaled(&self, scale: f64) -> Vec<f64> {
        rescale::rescale(&self.pos, scale)
    }
}

/// `k`, `repulsion` and `gravity` exactly as `_renormalize` derives them.
fn renormalize(mass: &[f32], params: Fa2ForceSimParams) -> (f64, f64, f64) {
    let k = SIM_SCALE / libm::cbrt(mass.len().max(1) as f64).max(1.0);
    // `float(mass.mean())`: numpy's pairwise f32 sum, then the f32 division by n, then the
    // f64 re-wrap. `max(n, 1)` in the reference guards `k`; `mean` is guarded by `if self.n`.
    let mean_mass = if mass.is_empty() {
        1.0
    } else {
        f64::from(pairwise_f32(mass) / mass.len() as f32)
    };
    let m2 = (mean_mass * mean_mass).max(MIN_MASS_POWER);
    let m3 = (mean_mass * mean_mass * mean_mass).max(MIN_MASS_POWER);
    let repulsion = params.scaling_ratio * (k * k / m3);
    let norm = GRAVITY_FIT * 2.0 * (SIM_SCALE * SIM_SCALE) / (k.max(MIN_K) * m2);
    (k, repulsion, params.gravity * GRAVITY_NORM * norm)
}
