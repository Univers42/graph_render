//! Barnes-Hut force layout: `manyBody` (charge) via the quadtree, `link` and `collide`
//! ported to Jacobi gathers with canonical pair orientation (devil C7), `center` as a
//! plain mean-and-translate. `prompt.md` §3.1, Phase 6 branch p6f, ported from
//! `/home/user/refs/npm/d3-force-3.0.0`. Split across `barnes_hut/{seed,sim,link,charge,
//! collide}.rs` for the house line cap — the same split `quadtree.rs`/`quadtree/tests.rs`
//! already uses, reported as a deviation from the branch's file list, which named
//! `barnes_hut.rs` alone.
//!
//! Deviates from d3-force in five documented ways (also recorded in
//! `docs/measurements/phase06-stress.md`):
//! 1. link and collide are Jacobi (not Gauss-Seidel) gathers (devil C7).
//! 2. `jiggle` is the counter-based hash in [`crate::rng`], not a seeded LCG (devil C8).
//! 3. link's degree bias uses the pair's lower/higher node index in place of the raw
//!    graph's own (now-discarded) source/target direction (`barnes_hut/link.rs`).
//! 4. seed positions centre on the origin, not a viewport (`barnes_hut/seed.rs`).
//! 5. there is no cluster force (Phase 10's node groups do not exist yet).

mod charge;
mod collide;
mod link;
mod seed;
mod settle;
mod sim;
mod step;

#[cfg(test)]
mod tests;

use super::params::{ForceParams, TICKS};
use crate::exec::Serial;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use sim::{How, Sim};

/// Which of the tick's range-kernel merges the negative control splits.
///
/// **A compiled-in parameter, never a `cfg` and never an environment read** — graph-core
/// reads no clock, no environment and no hardware, and the host supplies even the
/// mutation. One variant per pass, rather than one boolean for the whole tick, because the
/// control's job is to prove *a particular* kernel is compared: `GM_MUTATE_SPLIT_SUM=collide`
/// must go red even though the charge and link merges are honest, or the gate row would be
/// satisfied by a kernel nobody split.
///
/// The split is the shape a wrong partition of the outputs takes — one node's sum read
/// into another's — so a stage run with it set is a stage whose bytes no host would ship.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Split {
    /// No control: the stage itself.
    #[default]
    None,
    /// The many-body pass's merge reads the next node's delta too.
    Charge,
    /// The collide pass's merge does.
    Collide,
    /// The link pass's merge does.
    Link,
    /// Every merge above — what `GM_MUTATE_SPLIT_SUM=1` asks for.
    All,
}

impl Split {
    /// Whether the merge of `pass` is split under this control. `Split::All` covers every pass,
    /// which is why this is a method and not the `==` a single-variant control would need.
    pub fn splits(self, pass: Split) -> bool {
        self == pass || self == Split::All
    }

    /// The fewest seeds at which this control can move **anything** — the floor a gate row
    /// for it must reach.
    ///
    /// **Collide's is five, and that is measured, not chosen.** The gate's model is
    /// `2 + seed % 600` nodes; at two or three nodes, link and many-body have already pushed
    /// every pair past `2 * collideRadius` before collide runs, so collide finds no overlap,
    /// its deltas are all `0.0`, and *no* split of its merge can change a byte. Collide's
    /// control first bites at seed 4 (six nodes), which
    /// `barnes_hut/tests/kernels.rs::every_passs_own_control_bites_by_the_gate_s_fifth_seed`
    /// measures by running it seed by seed. Charge and link bite at seed 0.
    ///
    /// A gate row below this floor would exit 0 having corrupted nothing — a **vacuous
    /// pass**, which is worse than a red one because it reads as evidence. The gate refuses
    /// the run instead (exit 2, "could not run"); see
    /// `graph-cli/src/hashgate.rs`'s [`min_seeds`](Split::min_seeds) reader.
    pub const fn min_seeds(self) -> u32 {
        match self {
            Split::None | Split::Charge | Split::Link => 1,
            // `Split::All` includes collide, so it inherits collide's floor: a run of it
            // that reaches only seed 1 proves charge and link, and a row filed under "all
            // three" would claim collide as well.
            Split::Collide | Split::All => 5,
        }
    }
}

pub(crate) use settle::{Tier, golden_seed, settle};

/// Barnes-Hut approximated force layout (`prompt.md` §3.1).
///
/// Ponytail: force layouts are chaotic — the same graph with one node added or removed
/// is a different picture, not a perturbed one; there is no failing input narrower than
/// "any topology change". Direction: cosmetic-but-surprising, not silently wrong (the
/// stress metric, not visual stability, is what this layout is graded on). Escape hatch:
/// a fixed seed and this stage's own determinism — the same topology, run twice, settles
/// to the same geometry every time (`barnes_hut/tests.rs`'s
/// `the_same_topology_settles_to_the_same_geometry_run_to_run`).
pub struct BarnesHut;

impl Stage for BarnesHut {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.barnes_hut";

    /// The serial tier, which is the stage: [`run_with`](Self::run_with) over
    /// [`Serial`] with one worker, and the arm every other runner must hash-equal.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        Self::run_with(topology, params, &Serial, 1)
    }
}

impl BarnesHut {
    /// The passes of a tick that are handed to the [`crate::exec::Runner`] as range
    /// kernels, listed in the tick's own order: `link`, then `charge`, then `collide`,
    /// with `center` between the second and third and never threaded.
    ///
    /// The list exists because a measurement report states it: every speedup a threaded
    /// tier shows is this list's share of the stage, so a pass threaded without being
    /// listed here would silently misattribute a speedup. The array's **length** is what
    /// a test can check from outside the tick — the three kernels share an output type and
    /// a length, so a runner cannot tell them apart — and
    /// `barnes_hut/tests/kernels.rs::the_tick_hands_the_runner_one_call_per_listed_pass`
    /// fails if the two ever disagree in count. The **order** is a claim about
    /// [`Sim::tick`]'s body, written here for the reader; a test that could hold it would
    /// need the kernels to name themselves, which `StepRange` deliberately does not ask.
    pub const THREADED_PASSES: [&'static str; 3] = ["link", "charge", "collide"];

    /// The same layout, with the many-body pass handed to `runner` over `workers` workers.
    ///
    /// The point of the signature is what it does **not** change: the tick order, the
    /// `alpha` decay, the tree build, the link and collide passes and the integration are
    /// all the same code the serial stage runs, and only the division of the three
    /// partitioned passes differs. So `run_with(..., &Serial, 1)` is [`Stage::run`] by
    /// construction, and any other runner is a *schedule* of the same computation — which
    /// is the claim the N-way hash gate checks and `barnes_hut/tests.rs` pins at the unit
    /// level.
    ///
    /// `workers` below 2 is the serial path (see [`crate::exec::Runner`]), so a host
    /// reporting no threads gets the same bytes rather than a fast failure.
    pub fn run_with(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        Self::run_under(topology, params, runner, workers, Split::None)
    }

    /// [`run_with`](Self::run_with) with the negative control reachable, so the host can
    /// run a *deliberately wrong* tier and the gate must go red.
    ///
    /// Separate from [`run_with`](Self::run_with) rather than a defaulted argument on it
    /// because the default would be one value away from a stage that silently mutated
    /// itself — and a control that a caller can forget to pass is not a control, it is a
    /// fourth path nobody exercises.
    pub fn run_under(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
        split: Split,
    ) -> Result<Geometry, StageError> {
        let mut sim = Sim::new(topology, *params, 0);
        let mut deltas: Vec<(f64, f64)> = Vec::new();
        for _ in 0..TICKS {
            let mut how = How {
                runner,
                workers,
                deltas: &mut deltas,
                split,
            };
            sim.tick(&mut how);
        }
        let (x, y) = sim.positions();
        if x.iter().chain(y).any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: x.iter().map(|&v| v as f32).collect(),
                y: y.iter().map(|&v| v as f32).collect(),
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}
