//! Where a measurement's positions come from, and the measurement itself.
//!
//! Two position sets per size, because a solver's accuracy is not a property of the solver
//! alone: the *start* set is the engine's own golden-angle spiral, spread out and with no
//! pair closer than a cell, and the *settled* set is what [`SETTLE_TICKS`] Barnes-Hut ticks
//! leave, which is where the layout actually spends its life. A solver graded only on the
//! first is graded on a configuration the product never runs.

use super::exact::{self, Error, Exact, Law};
use super::solver::Solver;
use graph_core::Topology;
use graph_core::layout::force::{ForceParams, ForceSession, LiveParams};

/// Ticks a session runs to produce the `settled` set.
///
/// 100, not the frozen 112: the set is a *state*, and the state a layout sits in for most
/// of its run is the settled one. Caveat: a layout that has not converged by 100 ticks is
/// still being measured here, so `settled` means "after 100 ticks" and not "converged" —
/// which is why the number of ticks is a constant a reader can check rather than a word.
pub const SETTLE_TICKS: u32 = 100;

/// Which of the two position sets a row is about, in report order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Set {
    /// The session's own seed positions (`barnes_hut/seed.rs`'s golden-angle spiral).
    Start,
    /// The positions after [`SETTLE_TICKS`] Barnes-Hut ticks on the scale model.
    Settled,
}

impl Set {
    /// The name a table row prints.
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Settled => "settled",
        }
    }

    /// Both sets, in report order. The order is the report's, not a `HashMap`'s (D4).
    pub fn all() -> [Self; 2] {
        [Self::Start, Self::Settled]
    }
}

/// The `(x, y)` column pair a set is measured at.
pub type Positions = (Vec<f64>, Vec<f64>);

/// The two position sets at `n` nodes of the scale model: the seed positions, and the
/// positions after [`SETTLE_TICKS`] Barnes-Hut ticks.
///
/// The ticks run Barnes-Hut whatever solver is measured, because `settled` is a *state of
/// the graph* and a state reached by a different engine is a different state. The model is
/// the scale model every other size measurement uses (`bench/scale.rs`).
pub fn positions(topology: &Topology) -> [Positions; 2] {
    let mut session = frozen_session(topology);
    let start = columns(&session);
    session.step(SETTLE_TICKS);
    [start, columns(&session)]
}

fn frozen_session(topology: &Topology) -> ForceSession {
    ForceSession::from_frozen(topology, &ForceParams::default()).expect("frozen params are valid")
}

fn columns(session: &ForceSession) -> Positions {
    (session.xs().to_vec(), session.ys().to_vec())
}

/// The exact sum's law, read from the same [`ForceParams`] the solvers run under: the
/// charge and the two distance bounds are the many-body pass's whole input besides the
/// positions.
fn law(params: &LiveParams) -> Law {
    Law {
        dmin2: params.distance_min * params.distance_min,
        dmax2: params.distance_max * params.distance_max,
        charge: params.charge,
    }
}

/// One solver's force at `at`, through graph-core's own pass and nothing else.
pub fn solver_force(topology: &Topology, at: &Positions, solver: Solver) -> Vec<(f64, f64)> {
    let (xs, ys) = at;
    let session = ForceSession::from_positions(topology, LiveParams::default(), xs, ys)
        .expect("the columns came out of a session, so they are in range");
    // The mesh has no opening angle, so it reads `theta` as zero and the argument is
    // simply not a knob there: `Solver::Tree(0.0)` and `Solver::Mesh` differ in the engine
    // the session ticks with and in nothing else.
    let session = match solver {
        Solver::Mesh => session.with_particle_mesh(),
        Solver::Tree(_) => session,
    };
    let (vx, vy) = session.charge_deltas(solver.theta());
    vx.into_iter().zip(vy).collect()
}

/// One row of the table: a solver's error against the exact sum at one size and set.
#[derive(Debug, Clone)]
pub struct Row {
    /// Nodes measured.
    pub n: u32,
    /// Which position set.
    pub set: Set,
    /// Which solver.
    pub solver: Solver,
    /// The three error numbers.
    pub error: Error,
    /// Ordered pairs the exact sum skipped as coincident, at this set.
    pub coincident: u64,
    /// Barnes-Hut at the frozen `theta`'s relative RMS error at this same set: the bar
    /// `pass` is read against, carried on the row so a failing row can name it.
    pub baseline_rms: f64,
    /// Whether `error.rms` is at most `baseline_rms`.
    pub pass: bool,
}

/// Measures every `(set, solver)` pair at one size and returns the rows in report order.
///
/// The baseline is Barnes-Hut at the frozen `theta`: decision 2 of
/// `docs/decisions/obsidian-force.md` admits a many-body solver as Barnes-Hut's stand-in
/// only if it is no further from the exact sum than Barnes-Hut is, and "Barnes-Hut" there
/// means the frozen angle. Every row's `pass` is read against *that set's* baseline, never
/// against a number computed once and reused.
pub fn measure(topology: &Topology, n: u32, solvers: &[Solver]) -> Vec<Row> {
    let law = law(&LiveParams::default());
    let baseline = Solver::Tree(ForceParams::default().theta);
    let mut rows = Vec::new();
    for (set, at) in Set::all().into_iter().zip(positions(topology)) {
        let sum: Exact = exact::sum(&at.0, &at.1, law);
        let bar = exact::error(&solver_force(topology, &at, baseline), &sum.force).rms;
        for solver in solvers {
            let error = exact::error(&solver_force(topology, &at, *solver), &sum.force);
            rows.push(Row {
                n,
                set,
                solver: *solver,
                error,
                coincident: sum.coincident,
                baseline_rms: bar,
                pass: error.rms <= bar,
            });
        }
    }
    rows
}
