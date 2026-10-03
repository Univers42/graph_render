//! ForceAtlas2 as SciGraphs actually runs it: its own `ForceSim`, not networkx.
//!
//! This is `SciGraphs/core/scigraphs_core/mesh/layouts/simulation.py` (`ForceSim`,
//! `model='FA2'`) reached through `forceatlas.py:92-147` (`_forceatlas2_forcesim`), ported
//! for the path the conformance fixtures take. `layout.forceatlas2` stays the networkx
//! port: it is gated against networkx 3.6 by `oracle-fa2`, and re-pointing it here would
//! break its own oracle.
//!
//! ## What is ported
//!
//! `repulsion_mode` direct (`_pair_force`, `simulation.py:72-92`) — every fixture is under
//! `DIRECT_MAX = 400` nodes, where `_force_field` (`simulation.py:1000-1017`) takes the
//! direct branch whatever `repulsion_mode` says, and `_fa2_report_barnes_hut`
//! (`forceatlas.py:75-89`) prints exactly that. `f32` state (`DTYPE`, `simulation.py:12`),
//! the `k = scale / cbrt(n)` move cap (`simulation.py:376`), gravity scaled by
//! `_FA2_GRAVITY_NORM = 0.1` (`forceatlas.py:27`), and `sim.step(iterations)`
//! (`simulation.py:1056`).
//!
//! ## The dtype trace, because it decides the arithmetic
//!
//! Under NEP 50 a Python `float` is **weak** and stays `f32` against an `f32` array, but an
//! `np.float64` **scalar is not** and promotes the whole expression to `f64`. Which of the
//! two a constant is decides the dtype of a whole force term, so each is named here:
//!
//! - `self.k = scale / max(np.cbrt(max(n, 1)), 1.0)` is `np.float64`, therefore
//!   `self._repulsion_norm = k**2 / m3` and `self._gravity_norm` are `np.float64`, and
//!   therefore `self.repulsion` and `self.gravity` are `np.float64`. **The repulsion
//!   coefficient block and the whole of gravity are `f64`**, narrowed to `f32` only where
//!   they are stored (`out = np.empty(..., dtype=DTYPE)` at `:74`, `.astype(DTYPE)` at
//!   `:733`).
//! - `self.speed = float(np.clip(...))` is a Python `float`, so it is **weak** and
//!   `factor = self.speed / (1.0 + self.speed * np.sqrt(swing))` stays `f32` — and so do
//!   `disp` and `norm` with it, and `self.pos += disp` narrows nothing at all. **Reading
//!   `speed` as `f64` because `k` and `repulsion` are is the most expensive mistake
//!   available in this file**: it is a relative `6e-8` per move, which is one `f32` ULP, and
//!   by iteration 5 it had moved 12 of `tree-balanced`'s 45 coordinates. The one place `f64`
//!   does return is `cap / norm[over]`, because `cap` is `self.k`. See
//!   [`integrate`](self::integrate).
//! - `np.bincount(..., weights=...)` accumulates in `f64` whatever the weights' dtype
//!   (`simulation.py:721-722`), and `_attraction` narrows with `.astype(DTYPE)` at `:723`.
//! - `np.einsum` accumulates in the array's own dtype, so the `f32` `einsum`s stay `f32`.
//! - `total_swing = float(np.dot(self.mass, swing))` is a BLAS `sdot` on `f32` operands:
//!   an `f32` reduction whose result `float()` only re-wraps.
//! - `_fa2_rescale` is `f64` throughout: it is handed
//!   `np.asarray(sim.positions, dtype=np.float64)` (`forceatlas.py:147`), so the widening
//!   happens before the centring and the narrowing for the wire happens after.
//!
//! ## Reductions
//!
//! Named per expression in [`reduce`], with the measured order for each. Three of the nine
//! are BLAS and cannot be reproduced from a portable Rust; `pair_force.rs` and
//! `integrate.rs` each say which, and what the substitution costs — measured at zero for
//! both products and named as the row's residual cause for the `sdot`.
//!
//! ## Not ported
//!
//! `Ponytail (repulsion mode)`: this takes the direct all-pairs path at every `n`, where
//! the reference switches on `repulsion_mode` at `DIRECT_MAX = 400` — `'TREE'` to a quadtree
//! (`_repulsion_tree`, `simulation.py:532-552`, and only when `scigraphs_utils` ships one,
//! which the oracle image does not) and otherwise a near-field KD-tree plus a coarse
//! monopole grid (`_repulsion_near` + `_repulsion_far`, `:577-692`), measured by the
//! reference's own docstring at 5-13% median force error. Failing input: a graph over 400
//! nodes, where the two are different pictures and not perturbed ones. Direction: exact
//! repulsion, so the layout is *more* spread, never less. Escape hatch:
//! `layout.forceatlas2.barnes_hut`, and the conformance arm's fixtures, which are all under
//! the threshold.
//!
//! `Ponytail (planar start)`: the reference nudges a *planar* start out of the plane with
//! `rng.standard_normal(n) * 0.05k` (`simulation.py:340-344`), a ziggurat draw this port
//! does not have. Failing input: a start whose whole z column spans under `1e-6 * scale`,
//! which a seeded random start in `[-2.5, 2.5]` does not: the widest gap among `n` uniform
//! draws over that interval is over `1e-3` at `n = 3`. Direction: a planar start would stay
//! planar here and collapse in the reference's place. Escape hatch: none needed — every
//! caller of this layout draws its start from [`crate::rng::Pcg64`], which is uniform.
//!
//! `Ponytail (params)`: `strong_gravity`, `lin_log_mode`, `barnes_hut_optimize`,
//! `barnes_hut_theta` and `edge_weight_influence` are fixed at their `forceatlas.py`
//! defaults and are not knobs here. Three are read by no force term on this path (the last
//! is inert without edge weights, and `_build_edge_w` returns `None` outright at
//! `simulation.py:278-284`), and the other two select force laws this port does not
//! implement. Failing input: a caller who wants `lin_log_mode`, which would switch the
//! model to `LINLOG` (`simulation.py:310-311`). Escape hatch: the networkx port's
//! `Fa2Params` has no `lin_log` either; adding it is a new force law, not a knob.

mod attraction;
mod gravity;
mod integrate;
mod pair_force;
mod reduce;
mod rescale;
mod seed;
mod state;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::simple_graph;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use state::Sim;

/// ForceAtlas2 through SciGraphs' own `ForceSim` (`layout.forceatlas2.forcesim`).
///
/// Ponytail: force layouts are chaotic — the same graph with one node added or removed is
/// a different picture, not a perturbed one — so this is a *different layout* from
/// [`super::forceatlas2::ForceAtlas2`], not a variant of it. It exists because it is what
/// the `FORCEATLAS2` conformance row's reference actually runs.
pub struct ForceAtlas2ForceSim;

/// The parameters `apply_graph_layout` reaches `_forceatlas2_layout` with for `FORCEATLAS2`
/// at `props = None` (`forceatlas.py:150-153`, `dispatcher.py:62-64`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fa2ForceSimParams {
    /// `iterations`, the dispatcher's default (`dispatcher.py:14`), which reaches
    /// `sim.step(max(1, int(iterations)))` (`forceatlas.py:141`).
    pub iterations: u32,
    /// `scale`, the dispatcher's default, and the width `_fa2_rescale` fits the answer to.
    ///
    /// **Not the simulation's own scale.** `ForceSim` is constructed with
    /// `scale=_FA2_SIM_SCALE` (`forceatlas.py:29,130,138`), a module constant, so `k` and
    /// the two norms do not move when this does.
    pub scale: f64,
    /// `scaling_ratio`, the `_raw_repulsion` `ForceSim` renormalises by `n` and `scale`.
    pub scaling_ratio: f64,
    /// `gravity`, before `forceatlas.py:27` multiplies it by `0.1`.
    pub gravity: f64,
    /// `jitter_tolerance`, the numerator of the adaptive speed's target.
    pub jitter_tolerance: f64,
    /// Seeds the start positions through `default_rng(seed).random((n, 3))`
    /// (`simulation.py:1090`).
    ///
    /// The default is the seed the reference's own `FORCEATLAS2` row runs on: the first
    /// draw of `RandomState(981798123).randint(0, 2**31 - 1)` (`forceatlas.py:122`),
    /// measured at `1767573729` and pinned in `docs/measurements/sg-fa2-seed.md`.
    pub seed: u64,
}

impl Default for Fa2ForceSimParams {
    fn default() -> Self {
        Self {
            iterations: 50,
            scale: 5.0,
            scaling_ratio: 2.0,
            gravity: 1.0,
            jitter_tolerance: 1.0,
            seed: 1_767_573_729,
        }
    }
}

impl Stage for ForceAtlas2ForceSim {
    type Params = Fa2ForceSimParams;
    const ID: &'static str = "layout.forceatlas2.forcesim";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let graph = simple_graph(topology);
        let mut sim = Sim::new(topology.node_count() as usize, &graph, *params);
        sim.step(params.iterations);
        in_space(sim.rescaled(params.scale))
    }
}

/// The three narrowed columns, or the stage's refusal (D9) if any is not finite.
fn in_space(points: Vec<f64>) -> Result<Geometry, StageError> {
    if points.iter().any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let narrow = |c: usize| {
        points
            .iter()
            .skip(c)
            .step_by(3)
            .map(|&v| v as f32)
            .collect()
    };
    Ok(Geometry::in_space(
        NodeGeometry::Point {
            x: narrow(0),
            y: narrow(1),
        },
        EdgeGeometry::Line,
        Vec::new(),
        narrow(2),
    ))
}
