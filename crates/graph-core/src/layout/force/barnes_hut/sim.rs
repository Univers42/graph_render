//! The simulation state every force reads and writes: dense positions/velocities, the
//! two Barnes-Hut quadtrees (one over current positions for charge, one over projected
//! `x+vx` positions for collide, matching `manyBody.js`/`collide.js`'s own quadtree
//! keys), and the per-pass scratch buffers link/collide gather into before merging
//! (devil C7's "double buffered": a pass reads last pass's `vx`/`vy` in full, never one
//! it has itself begun to overwrite — see `link.rs`, `collide.rs`).
//!
//! **One tick, driven two ways.** [`Sim::tick`] takes a [`How`] — the tier choice — and is
//! the *only* tick: the frozen stage, a threaded run and a live
//! [`ForceSession`](crate::layout::force::ForceSession) all call this function, so a
//! session step and a batch step are the same computation and not two that agree.

use super::seed::golden_spiral;
use crate::index::Topology;
use crate::layout::force::LiveParams;
use crate::layout::force::quadtree::Quadtree;
use crate::layout::force::session::gravity;
use crate::layout::force::{SimpleGraph, simple_graph};

/// The tier choice for one run, bundled so [`Sim::tick`] stays under the four-parameter cap
/// (`refactor-rust.md`) and so the scratch buffer travels with the runner that fills it — a
/// caller cannot pair a runner with another caller's buffer by accident.
///
/// `pub(in crate::layout::force)` rather than `pub(super)` because a live
/// [`ForceSession`](crate::layout::force::ForceSession) builds one for its own serial
/// step: the interactive path is a *schedule* of this tick too, not a separate tick.
pub(in crate::layout::force) struct How<'a, R: crate::exec::Runner> {
    /// Who runs the ranges.
    pub(in crate::layout::force) runner: &'a R,
    /// How many workers it may use.
    pub(in crate::layout::force) workers: u32,
    /// The scratch the gathers write, reused across ticks and across the passes within a
    /// tick: every range kernel's output is a per-node `(dvx, dvy)` column, so one buffer
    /// of `n` serves all three.
    pub(in crate::layout::force) deltas: &'a mut Vec<(f64, f64)>,
    /// The negative control: which merge, if any, reads a neighbouring node's delta.
    pub(in crate::layout::force) split: super::Split,
}

/// Widened from `pub(super)` to `pub(in crate::layout::force)` for the same reason as
/// [`How`]: the session is a *sibling* of `barnes_hut` under `force`, and it owns this
/// state rather than copying it. It stays crate-internal — the columns a caller reads are
/// [`ForceSession::xs`](crate::layout::force::ForceSession::xs) and its `ys`.
pub(in crate::layout::force) struct Sim {
    pub(in crate::layout::force) graph: SimpleGraph,
    pub(in crate::layout::force) params: LiveParams,
    pub(in crate::layout::force) seed: u32,
    pub(in crate::layout::force) tick_no: u32,
    pub(in crate::layout::force) alpha: f64,
    /// d3's `alphaTarget` (`simulation.js:21`, applied at `:45`). Zero is the frozen
    /// layout's value and the only one that lets a run settle.
    pub(in crate::layout::force) alpha_target: f64,
    pub(in crate::layout::force) x: Vec<f64>,
    pub(in crate::layout::force) y: Vec<f64>,
    pub(in crate::layout::force) vx: Vec<f64>,
    pub(in crate::layout::force) vy: Vec<f64>,
    /// `node.fx`, `node.fy` (`simulation.js:54-56`). `None` is a free axis, and with every
    /// axis free the integration tail is the two statements it has always been.
    pub(in crate::layout::force) fx: Vec<Option<f64>>,
    pub(in crate::layout::force) fy: Vec<Option<f64>>,
    pub(in crate::layout::force) px: Vec<f64>,
    pub(in crate::layout::force) py: Vec<f64>,
    pub(in crate::layout::force) charge_tree: Quadtree,
    pub(in crate::layout::force) collide_tree: Quadtree,
    pub(super) bodies: Vec<super::charge::Body>,
    pub(in crate::layout::force) link_distance: Vec<f64>,
    pub(in crate::layout::force) link_strength: Vec<f64>,
    pub(in crate::layout::force) link_bias: Vec<f64>,
    /// Each simple edge's force this tick, the link pass's scratch.
    pub(super) link_forces: Vec<(f64, f64)>,
}

#[cfg(test)]
mod tests;

impl Sim {
    pub(in crate::layout::force) fn new(
        topology: &Topology,
        params: LiveParams,
        seed: u32,
    ) -> Self {
        let graph = simple_graph(topology);
        let (x, y) = golden_spiral(topology.node_count());
        Self::from_parts(graph, params, seed, (x, y))
    }

    /// A simulation over `graph` starting from the given positions (one per node).
    pub(in crate::layout::force) fn from_parts(
        graph: SimpleGraph,
        params: LiveParams,
        seed: u32,
        (x, y): (Vec<f64>, Vec<f64>),
    ) -> Self {
        let n = x.len();
        let (link_distance, link_strength, link_bias) = super::link::geometry(&graph, &params);
        Self {
            alpha: params.initial_alpha,
            alpha_target: 0.0,
            graph,
            params,
            seed,
            tick_no: 0,
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            fx: vec![None; n],
            fy: vec![None; n],
            px: vec![0.0; n],
            py: vec![0.0; n],
            charge_tree: Quadtree::default(),
            collide_tree: Quadtree::default(),
            bodies: Vec::new(),
            x,
            y,
            link_distance,
            link_strength,
            link_bias,
            link_forces: Vec::new(),
        }
    }

    /// How many node columns a row may name.
    pub(in crate::layout::force) fn rows(&self) -> u32 {
        self.x.len() as u32
    }

    /// Replaces the parameters and the per-link geometry derived from them, and nothing
    /// else: the columns, the velocities and the pins are the run's, not the parameters'.
    pub(in crate::layout::force) fn set_params(&mut self, params: LiveParams) {
        let (distance, strength, bias) = super::link::geometry(&self.graph, &params);
        (self.link_distance, self.link_strength, self.link_bias) = (distance, strength, bias);
        self.params = params;
    }

    /// The capacity of every buffer a tick refills, for the test that a steady-state run
    /// allocates nothing. The walks are stackless, so this is every buffer a tick owns.
    #[cfg(test)]
    pub(in crate::layout::force) fn scratch_capacities(&self) -> Vec<usize> {
        vec![
            self.bodies.capacity(),
            self.charge_tree.capacity(),
            self.collide_tree.capacity(),
            self.link_forces.capacity(),
        ]
    }

    /// One tick with the three gathered passes divided by `runner` over `workers` workers:
    /// `alpha` decays first (`simulation.js:45`, in d3's own form), then link, many-body,
    /// center, collide and gravity run in the engine's registration order
    /// (`forceLayout.ts`'s `.force("link",...).force("charge",...).force("center",...)
    /// .force("collide",...)`, with gravity after them: d3 runs forces in registration
    /// order, `simulation.js:47-49`, and an anchoring force belongs after the interactions
    /// it is damping), then velocities integrate into position.
    ///
    /// The **only** tier-sensitive lines are those three passes' range kernels:
    /// everything else — the decay, center, integrate, and the pass order itself — is the
    /// same straight-line code in every tier. That is deliberate and it is the whole safety
    /// argument: the sequence of reads and writes per node is unchanged, so tick `t + 1`
    /// sees exactly what tick `t` left whether one thread or seven made it. Each pass's own
    /// prologue (charge's and collide's tree build, the bottom-up aggregate) stays
    /// single-threaded and ahead of its ranges, as `phase-11-compute-tiers.md` step 2
    /// prescribes.
    ///
    /// `deltas` is the caller's scratch, reused across ticks so a steady-state run
    /// allocates nothing here — one buffer for the whole layout, not one per tick.
    pub(in crate::layout::force) fn tick<R: crate::exec::Runner>(&mut self, how: &mut How<'_, R>) {
        let How {
            runner,
            workers,
            deltas,
            split,
        } = how;
        let (runner, workers, split) = (*runner, *workers, *split);
        // d3's own form (`simulation.js:45`). With `alpha_target == 0.0`, `0.0 - alpha` is
        // exactly `-alpha` for every positive alpha a cooling schedule produces, so this is
        // the frozen layout's `alpha += -alpha * alpha_decay`, bit for bit.
        self.alpha += (self.alpha_target - self.alpha) * self.params.alpha_decay;
        super::link::apply_with(
            self,
            runner,
            workers,
            deltas,
            split.splits(super::Split::Link),
        );
        super::charge::apply_with(
            self,
            runner,
            workers,
            deltas,
            split.splits(super::Split::Charge),
        );
        self.center();
        super::collide::apply_with(
            self,
            runner,
            workers,
            deltas,
            split.splits(super::Split::Collide),
        );
        // **The one statement in the crate that says a force is *off* rather than zero.**
        // `(0 - x) * 0.0` is `+0.0` for `x < 0` and `-0.0` for `x > 0`, so applying the
        // force at zero strength would reach a different `f64` than applying nothing —
        // `session/gravity.rs::applying_it_at_zero_is_not_the_same_bytes` is the arithmetic.
        if self.params.gravity > 0.0 {
            gravity::apply(self);
        }
        self.integrate();
        self.tick_no += 1;
    }

    /// `center.js`: shifts every position by the mean, toward the origin — position, not
    /// velocity, and with no `alpha` scaling (`center.js`'s `force()` takes no `alpha`).
    pub(in crate::layout::force) fn center(&mut self) {
        let Some((dx, dy)) = self.center_shift() else {
            return;
        };
        for i in 0..self.x.len() {
            self.x[i] -= dx;
            self.y[i] -= dy;
        }
    }

    /// What [`center`](Self::center) subtracts: the mean position, scaled by the
    /// centering strength, folded in node order. `None` for no nodes.
    pub(in crate::layout::force) fn center_shift(&self) -> Option<(f64, f64)> {
        let n = self.x.len();
        if n == 0 {
            return None;
        }
        let (sx, sy) = self
            .x
            .iter()
            .zip(&self.y)
            .fold((0.0, 0.0), |(a, b), (&x, &y)| (a + x, b + y));
        let strength = self.params.center_strength;
        Some(((sx / n as f64) * strength, (sy / n as f64) * strength))
    }

    /// `simulation.js`'s own tick tail: a pinned axis is *placed* and its velocity zeroed
    /// (`simulation.js:53-56`), a free axis integrates `node.x += node.vx *= velocityDecay`.
    ///
    /// The two free-axis statements are the same two, in the same order, as before pins
    /// existed — so a session with nothing pinned is bit-identical to the frozen layout,
    /// which is what keeps `session/tests/golden.rs`'s 65 digests valid.
    ///
    /// `pub(in crate::layout::force)` rather than private because the phase-11 range
    /// kernel's tests drive the same pass sequence the tick does
    /// ([`crate::layout::force::barnes_hut::tests`]), and a test that rebuilt the tick in
    /// its own words would be testing its own arithmetic rather than the kernel's.
    pub(in crate::layout::force) fn integrate(&mut self) {
        let decay = self.params.velocity_decay;
        for i in 0..self.x.len() {
            match self.fx[i] {
                Some(fx) => {
                    self.x[i] = fx;
                    self.vx[i] = 0.0;
                }
                None => {
                    self.vx[i] *= decay;
                    self.x[i] += self.vx[i];
                }
            }
            match self.fy[i] {
                Some(fy) => {
                    self.y[i] = fy;
                    self.vy[i] = 0.0;
                }
                None => {
                    self.vy[i] *= decay;
                    self.y[i] += self.vy[i];
                }
            }
        }
    }
}
