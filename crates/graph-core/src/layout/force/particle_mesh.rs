//! Particle-mesh force layout: Barnes-Hut's tick with the two passes that walk a quadtree
//! replaced by grid methods, so a tick is `O(n + P² log P)` rather than `O(n log n)` with
//! a large constant in tree walks.
//!
//! - **Many-body** deposits each node's unit charge on a `P × P` mesh with cloud-in-cell
//!   weights, convolves it with d3's `manyBody` law by FFT, and reads the field back with
//!   the same weights (`particle_mesh/{frame,kernel,mesh,fft,charge}.rs`).
//! - **Collide** sorts the projected positions into a hashed cell list one diameter wide
//!   and resolves the nine neighbour cells (`particle_mesh/collide.rs`).
//! - **Link and center** are Barnes-Hut's own code, called, not copied. The velocity merges
//!   and the integrate are its expressions as range passes (`particle_mesh/motion.rs`).
//!
//! A different layout id from Barnes-Hut, not a tier of it: the mesh changes the bytes.
//! It is deterministic in the same sense, one tier's bytes per run, native and wasm32
//! alike, and its own entry in the hash gate.
//!
//! Caveat: below about two mesh cells the charge force is smoothed (`particle_mesh/mesh.rs`),
//! so two close nodes repel less than under Barnes-Hut; link and collide own that range.
//! Above it the field is the law's direct sum, not a `theta` approximation of it.
//!
//! The stage is a [`ForceSession`] ticked here
//! ([`with_particle_mesh`](ForceSession::with_particle_mesh)), stepped [`TICKS`] times: the
//! live session and the frozen layout run one tick, as for Barnes-Hut.

mod charge;
mod collide;
mod deposit;
mod fft;
mod frame;
mod kernel;
mod mesh;
mod motion;

#[cfg(test)]
mod tests;

use super::barnes_hut::sim::{How, Sim};
use super::barnes_hut::{Split, link};
use super::params::{ForceParams, TICKS};
use super::session::gravity;
use super::{ForceSession, planar_points};
use crate::exec::{Runner, Serial};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
pub(in crate::layout::force) use mesh::Mesh;
use motion::Gathered;

/// Particle-mesh force layout.
///
/// Ponytail: the mesh side is capped at 1024, so past about 1M nodes a cell holds more
/// than one node on average and the short-range charge is the smoothed part of the force,
/// not a sliver of it. Direction: a layout less spread at small scale than Barnes-Hut's,
/// graded by the stress row rather than hidden. Escape hatch: Barnes-Hut, which keeps the
/// pairwise law at every range.
pub struct ParticleMesh;

impl Stage for ParticleMesh {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.particle_mesh";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        Self::run_with(topology, params, &Serial, 1)
    }
}

impl ParticleMesh {
    /// The layout with its gathers divided by `runner` over `workers` workers; every
    /// runner is a schedule of the same bytes, as for Barnes-Hut.
    pub fn run_with(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        Self::run_under(topology, params, runner, workers, Split::None)
    }

    /// [`run_with`](Self::run_with) with the negative control reachable, so the host can
    /// run a *deliberately wrong* tier and the gate must go red.
    ///
    /// Separate from [`run_with`](Self::run_with) for Barnes-Hut's reason, quoted: the
    /// default would be one value away from a stage that silently mutated itself, and a
    /// control a caller can forget to pass is not a control. The mesh needs this as much as
    /// Barnes-Hut does — its `link`, `charge` and `collide` passes each read a
    /// [`Split`] out of the tick's `How`, and the stage used to build that `How` with
    /// [`Split::None`] hard-coded, so nothing could reach them at all.
    pub fn run_under(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl Runner,
        workers: u32,
        split: Split,
    ) -> Result<Geometry, StageError> {
        let mut run = ForceSession::from_frozen(topology, params)?.with_particle_mesh();
        run.step_under(runner, workers, split, TICKS);
        planar_points(run.xs(), run.ys())
    }
}

/// Barnes-Hut's tick order with the mesh passes in place of the tree passes: decay, link,
/// many-body, center, collide, gravity, integrate.
pub(in crate::layout::force) fn tick<R: Runner>(
    sim: &mut Sim,
    mesh: &mut Mesh,
    how: &mut How<'_, R>,
) {
    sim.alpha += (sim.alpha_target - sim.alpha) * sim.params.alpha_decay;
    let split = how.split.splits(Split::Link);
    link::apply_with(sim, how.runner, how.workers, how.deltas, split);
    charge::apply(sim, mesh, how);
    sim.center();
    let collided = collide::apply(sim, &mut mesh.grid, how);
    // Skipped at zero as in `barnes_hut/sim.rs`: `(0 - x) * 0.0` is a signed zero that
    // changes the bytes (`session/gravity.rs`). Collide's push is merged after this one here
    // and before it there, so the sums round differently: these are the mesh's bytes.
    if sim.params.gravity > 0.0 {
        gravity::apply(sim);
    }
    let gathered = Gathered {
        deltas: how.deltas,
        slot: &mesh.grid.slot,
        split: how.split.splits(Split::Collide),
    };
    motion::integrate(sim, collided.then_some(gathered), (how.runner, how.workers));
    sim.tick_no += 1;
}
