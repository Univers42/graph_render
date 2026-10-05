//! The generator and the two states, in that order: one seeded model, the mesh's own
//! spiral, and [`SETTLE_TICKS`] ticks of the mesh's own tick.
//!
//! Caveat: [`SETTLE_TICKS`] is 100, not the frozen `TICKS = 112`, because 100 is the
//! number every other measurement in this repo calls settled
//! (`mb_fidelity/measure.rs:19`). A layout that has not converged by 100 ticks is still
//! being measured; "settled" here means "after 100 ticks", which is why it is a constant
//! and not a word.

use graph_core::REFERENCE_DEGREE;
use graph_core::layout::force::{ForceParams, ForceSession, MeshProbe};
use graph_core::{index_model, seeded_model};

/// The seed every GPU fixture uses: one model, so a fixture's identity is its node count.
pub const SEED: u32 = 0;

/// Ticks from the start state to the settled one.
pub const SETTLE_TICKS: u32 = 100;

/// The node counts the decision record names (`gpu-force-tier.md:73`), plus 1M: the
/// charge ceiling is *derived at 1M* and the stress row measures there, so a fixture the
/// tier never emits would make both a bound and a measurement a statement about nothing.
pub const SIZES: [u32; 4] = [1_000, 10_000, 50_000, 1_000_000];

/// Which of the two position sets a fixture is about, in wire order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The session's own seed positions, the engine's golden-angle spiral.
    Start,
    /// The positions after [`SETTLE_TICKS`] ticks of the mesh.
    Settled,
}

impl State {
    /// The name a file and a header field carry.
    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Settled => "settled",
        }
    }

    /// The wire word for this state (`0` start, `1` settled).
    pub fn word(self) -> u32 {
        match self {
            Self::Start => 0,
            Self::Settled => 1,
        }
    }

    /// Both states, in wire order.
    pub fn all() -> [State; 2] {
        [State::Start, State::Settled]
    }

    /// The state a fixture file named `mesh-<tag>-<state>.gmfx` is about.
    #[cfg(test)]
    pub fn of(name: &str) -> Option<State> {
        State::all()
            .into_iter()
            .find(|s| name.ends_with(&format!("-{}.gmfx", s.label())))
    }
}

/// The short size tag a file name carries: `1k`, `10k`, `50k`, `1m`.
pub fn size_tag(n: u32) -> String {
    match n {
        1_000 | 1_000_000 => format!("{}k", n / 1_000),
        _ => format!("{n}"),
    }
}

/// The fixture file name for one case: `mesh-<tag>-<state>.gmfx`.
pub fn file_name(n: u32, state: State) -> String {
    format!("mesh-{}-{}.gmfx", size_tag(n), state.label())
}

/// One mesh session over the seeded model at `n` nodes, at the frozen parameter set.
///
/// The frozen set and not [`graph_core::layout::force::LiveParams`]'s default: the frozen
/// constructor is the call `ParticleMesh::run_under` makes (`particle_mesh.rs:109`), so the
/// fixture is the layout the hash gate hashes, and the two parameter sets agree only because
/// `LiveParams::from(ForceParams)` copies every field (`session/live_params.rs:172-190`) —
/// the frozen path is the one that cannot drift.
pub fn session(n: u32) -> Result<ForceSession, String> {
    let (nodes, edges) = seeded_model(SEED, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| format!("reindexing: {e}"))?;
    ForceSession::from_frozen(&topology, &ForceParams::default())
        .map(|s| s.with_particle_mesh())
        .map_err(|e| e.to_string())
}

/// One case's whole reading: the session's own positions at its state, and the probe over
/// them. Settling happens here and nowhere else, so the start state is the seed and the
/// settled state is that seed after exactly [`SETTLE_TICKS`] ticks.
pub fn case(n: u32, state: State) -> Result<(ForceSession, MeshProbe), String> {
    let mut session = session(n)?;
    if state == State::Settled {
        session.step(SETTLE_TICKS);
    }
    let probe = session
        .mesh_probe()
        .ok_or_else(|| format!("n {n} {}: no field to solve", state.label()))?;
    Ok((session, probe))
}
