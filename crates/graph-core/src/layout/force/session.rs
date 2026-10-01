//! The **live** force session: the state a force simulation is when a caller drives it,
//! rather than a function that runs 112 ticks and returns one picture.
//!
//! # What this is, precisely
//!
//! The frozen layout is not a second implementation next to this one. It is *this*, in its
//! degenerate case: **a default [`LiveParams`], no pins, `alpha_target` 0, stepped
//! [`TICKS`](crate::layout::force::params::TICKS) times** — which is what
//! [`BarnesHut`](crate::layout::force::BarnesHut) now runs, and what
//! `session/tests/m1a.rs` pins byte for byte against the 65 digests taken from the stage
//! *before* this refactor. The forces, the quadtree and the scratch buffers are the ones
//! the frozen layout already hashed; there is one tick, in
//! [`barnes_hut::sim`](crate::layout::force::BarnesHut), and this session owns it.
//!
//! # The semantics, and where they come from
//!
//! Every one of them is d3's own, ported from the pinned d3-force oracle
//! (see `prompts/REFERENCES.md`, Tier 1):
//!
//! | What | d3 |
//! |---|---|
//! | `alpha += (alphaTarget - alpha) * alphaDecay` | `simulation.js:45` |
//! | a pin places the node and zeroes its velocity | `simulation.js:53-56` |
//! | a free node integrates `x += vx *= velocityDecay` | `simulation.js:53,55` |
//! | gravity, `(0 - x) * strength * alpha` | `x.js:13`, `y.js:13` (`forceX(0)`/`forceY(0)`) |
//! | `velocityDecay` is `1 - this` | `simulation.js:118` |
//! | center translates the mean, unscaled by `alpha` | `center.js:18-20` |
//!
//! Two places where this motor is deliberately *not* d3, both already true before it
//! became a session: link and collide are Jacobi gathers where d3 scatters in visit order
//! (devil C7), and `jiggle` is a counter-based hash rather than d3's seeded LCG (devil
//! C8). `session/tests/m1c.rs`'s digest is what says the composed result is stable.
//!
//! One place where this is deliberately stricter than d3: **settled** is
//! `alpha < alpha_min && alpha_target == 0`, where d3's own stop condition is
//! `alpha < alphaMin` alone (`simulation.js:33`). A layout being deliberately held hot by
//! a non-zero target is not a layout that has stopped moving, and reporting it as settled
//! would be a lie to the caller driving the loop.
//!
//! # What is refused, and what is not
//!
//! [`LiveParams`] is range-checked and never clamped (`live_params.rs`), and so are a warm
//! start's columns and a [`NodeRow`]. `reheat` and `set_alpha_target` are the two setters
//! that are not, and each says why on its own doc comment.

mod error;
pub(in crate::layout::force) mod gravity;
mod live_params;
mod pin;

#[cfg(test)]
mod tests;

pub use error::SessionError;
pub use live_params::LiveParams;
pub use pin::NodeRow;

use self::live_params::{ALPHA, ALPHA_TARGET};
use crate::exec::{Runner, Serial};
use crate::index::Topology;
use crate::layout::force::barnes_hut::Split;
use crate::layout::force::barnes_hut::sim::Sim;
use crate::layout::force::params::ForceParams;
use crate::stage::StageError;

/// The seed every session's coincidence guard starts from. The counter hash's only job is
/// separating two nodes at *exactly* the same point, which the golden-spiral seed does not
/// produce and a warm start may; the seed is not a source of variety here, and a session
/// that took one would be a session whose output depends on a number the layout never
/// uses.
const SEED: u32 = 0;

/// A force simulation a caller drives: pins it, reheats it, stops it, reads it.
///
/// Pure — no I/O, no clock, no ambient state (D8) — and deterministic: the same topology,
/// the same commands and the same parameters give the same bytes on every target, which
/// `session/tests/m1a.rs`–`m1e.rs` pin.
///
/// **It owns the one [`Sim`], and [`step`](Self::step) is the batch stage's tick under a
/// serial [`How`]** — not a second loop. `BarnesHut::run_under` calls
/// [`step_under`](Self::step_under) with the host's runner and worker count; this type's
/// own [`step`](Self::step) is that call with `Serial` and one worker. The difference
/// between an interactive step and a batch step is therefore *only* the schedule, which is
/// the claim `session/tests/m1b.rs` and the 4-way hash gate both check.
pub struct ForceSession {
    sim: Sim,
    /// The tick's scratch, owned here so a session steps without the caller having to hand
    /// it a buffer — and reused across ticks, so a steady-state run allocates nothing.
    deltas: Vec<(f64, f64)>,
}

/// What one call to [`ForceSession::step`] did.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StepReport {
    /// `alpha` after the ticks that ran.
    pub alpha: f64,
    /// `alpha` fell below `alpha_min` with no target holding it up: the cooling schedule has
    /// reached the point d3 stops at (`simulation.js:33`), so another tick scales every
    /// force by an alpha too small to see.
    pub settled: bool,
    /// How many ticks ran: the argument, always. A chunked caller adds these up.
    pub ticks_run: u32,
}

impl ForceSession {
    /// A session over `topology` at `params`, seeded on the engine's own golden-angle
    /// spiral (`barnes_hut/seed.rs`). Refused when a parameter is not in range
    /// ([`SessionError`]) — a session is never built with parameters it would refuse later.
    pub fn new(topology: &Topology, params: LiveParams) -> Result<Self, SessionError> {
        params.validate()?;
        Ok(Self::seeded(topology, params))
    }

    /// A session at the **frozen** parameter set, for the frozen stage.
    ///
    /// Its acceptance is finiteness only: the frozen stage hashes over a parameter set that
    /// predates the live ranges, so re-deriving them here would refuse settings it has
    /// always run. The live setters keep the ranges.
    pub fn from_frozen(topology: &Topology, params: &ForceParams) -> Result<Self, SessionError> {
        let params = LiveParams::from(*params);
        params.validate_finite()?;
        Ok(Self::seeded(topology, params))
    }

    /// The already-validated half of both constructors.
    fn seeded(topology: &Topology, params: LiveParams) -> Self {
        Self {
            sim: Sim::new(topology, params, SEED),
            deltas: Vec::new(),
        }
    }

    /// A session that starts from `xs`/`ys` rather than from the spiral: the warm start a
    /// caller needs to nudge a picture it already has, or to continue one it stored.
    ///
    /// Both columns must hold exactly one value per node and be finite; the velocities
    /// start at rest, because a position is not a velocity and inventing one here would
    /// make the result depend on what the session happened to be before.
    pub fn from_positions(
        topology: &Topology,
        params: LiveParams,
        xs: &[f64],
        ys: &[f64],
    ) -> Result<Self, SessionError> {
        let mut session = Self::new(topology, params)?;
        session.set_positions(xs, ys)?;
        Ok(session)
    }

    /// Runs `ticks` ticks and says what happened. The tick count is the *only* argument
    /// that changes the result: `step(112)`, `112 × step(1)` and `7 × step(16)` are the
    /// same bytes (`session/tests/m1b.rs`), which is what lets one code path serve both a
    /// batch settle and an interactive loop.
    pub fn step(&mut self, ticks: u32) -> StepReport {
        self.step_under(&Serial, 1, Split::None, ticks)
    }

    /// [`step`](Self::step) with the tier chosen by the caller: the same tick, with the
    /// three gathered passes divided by `runner` over `workers` workers.
    ///
    /// **This is the batch stage's path, not a parallel one.** `BarnesHut::run_under`
    /// builds a frozen session and calls this, so the 112-tick layout and an interactive
    /// loop execute one function; only the `How` differs. That is what makes a session step
    /// and a batch step the same bytes rather than two implementations that agree today.
    pub(crate) fn step_under<R: Runner>(
        &mut self,
        runner: &R,
        workers: u32,
        split: Split,
        ticks: u32,
    ) -> StepReport {
        for _ in 0..ticks {
            let mut how = crate::layout::force::barnes_hut::sim::How {
                runner,
                workers,
                deltas: &mut self.deltas,
                split,
            };
            self.sim.tick(&mut how);
        }
        self.report(ticks)
    }

    /// What the ticks that just ran did. Separate from the loop so the schedule and the
    /// verdict are two readable things rather than one.
    fn report(&self, ticks: u32) -> StepReport {
        let alpha = self.sim.alpha;
        StepReport {
            alpha,
            settled: alpha < self.sim.params.alpha_min && self.sim.alpha_target == 0.0,
            ticks_run: ticks,
        }
    }

    /// Replaces the parameters mid-run, recomputing only the per-link geometry that
    /// depends on them. Refused — leaving the session exactly as it was — when any field is
    /// non-finite or outside its range; see [`LiveParams::validate`].
    pub fn set_params(&mut self, params: LiveParams) -> Result<(), SessionError> {
        params.validate()?;
        self.sim.set_params(params);
        Ok(())
    }

    /// The `x` column, in row order, as a snapshot's positions are read.
    pub fn xs(&self) -> &[f64] {
        &self.sim.x
    }

    /// The `y` column, in row order.
    pub fn ys(&self) -> &[f64] {
        &self.sim.y
    }

    /// The parameters in force, field for field.
    pub fn params(&self) -> LiveParams {
        self.sim.params
    }

    /// The cooling schedule's current value, as the last [`step`](Self::step) reported it.
    pub fn alpha(&self) -> f64 {
        self.sim.alpha
    }

    /// Sets `alpha` outright — d3's own `alpha(_)` setter (`simulation.js:101-104`), and
    /// the verb behind "the user moved something, run it again". The value is taken
    /// exactly and never clamped, so the next [`step`](Self::step) cools it from wherever it
    /// was put; a value that is not finite or is outside `0..=1` is refused
    /// ([`SessionError`]) with the session left exactly as it was.
    pub fn reheat(&mut self, alpha: f64) -> Result<(), SessionError> {
        ALPHA.check(alpha)?;
        self.sim.alpha = alpha;
        Ok(())
    }

    /// d3's `alphaTarget(_)` (`simulation.js:113-116`): the value `alpha` moves *toward*
    /// instead of toward zero. Zero is the frozen layout's and the only value a run can
    /// settle at; anything else holds the layout hot, which is the point, and which
    /// [`StepReport::settled`] then refuses to call settled.
    ///
    /// The value is taken exactly and never clamped — a clamp would be a lie the caller
    /// cannot see, and this schedule is what a caller reads back. The upper end is open:
    /// a target of exactly 1 holds the layout at full heat forever without ever settling,
    /// which is a caller bug rather than a setting, so `0..=1` is refused where
    /// [`reheat`](Self::reheat) takes it.
    pub fn set_alpha_target(&mut self, target: f64) -> Result<(), SessionError> {
        ALPHA_TARGET.check_open(target)?;
        self.sim.alpha_target = target;
        Ok(())
    }

    /// How many node columns a [`NodeRow`] may name.
    fn rows(&self) -> u32 {
        self.sim.rows()
    }

    /// The capacity of every buffer the tick loop refills, for the test that a tick
    /// allocates nothing once the layout has reached steady state.
    #[cfg(test)]
    pub(crate) fn scratch_capacities(&self) -> Vec<usize> {
        self.sim.scratch_capacities()
    }

    /// Replaces both position columns, and the velocities with zeros.
    fn set_positions(&mut self, xs: &[f64], ys: &[f64]) -> Result<(), SessionError> {
        self.check_column("xs", xs)?;
        self.check_column("ys", ys)?;
        self.sim.x.copy_from_slice(xs);
        self.sim.y.copy_from_slice(ys);
        self.sim.vx.iter_mut().for_each(|v| *v = 0.0);
        self.sim.vy.iter_mut().for_each(|v| *v = 0.0);
        Ok(())
    }

    /// One warm-start column: the right length, and finite (D9 — wasm32 does not pin a
    /// NaN's bits).
    fn check_column(&self, column: &'static str, values: &[f64]) -> Result<(), SessionError> {
        let nodes = self.rows();
        if values.len() as u64 != u64::from(nodes) {
            return Err(SessionError::ColumnLength {
                column,
                got: values.len() as u64,
                nodes,
            });
        }
        if let Some(_bad) = values.iter().position(|v| !v.is_finite()) {
            return Err(SessionError::NonFinite { field: column });
        }
        Ok(())
    }
}

impl From<SessionError> for StageError {
    /// The frozen stage is a session with the frozen parameters, so a refusal to build one
    /// is a refusal to run the stage. `OutOfRange` carries its rule as a `&'static str`
    /// precisely so this conversion loses nothing.
    ///
    /// The other two variants cannot arise on the frozen path at all — a stage builds its
    /// own session and addresses no row — and `StageError::Param`'s rule is a
    /// `&'static str` with nowhere to format the numbers into. The numbers are in
    /// [`SessionError`], which is what a session's own caller sees and what its `Display`
    /// prints.
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::NonFinite { field } => Self::NonFinite { column: field },
            SessionError::OutOfRange { field, rule } => Self::Param { name: field, rule },
            SessionError::ColumnLength { .. } => Self::Param {
                name: "force session columns",
                rule: "one finite value per node",
            },
            SessionError::NoSuchRow { .. } => Self::Param {
                name: "force session row",
                rule: "a row inside the node columns",
            },
        }
    }
}
