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
//! Gravity and pins are not wired: the stage runs the frozen parameters, which have
//! neither, and [`ParticleMeshRun`] is that stage's run, not a live session.

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
use super::{LiveParams, SessionError, planar_points};
use crate::exec::{Runner, Serial};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use mesh::Mesh;
use motion::Gathered;

/// The seed the jiggle reads, the frozen stage's as for Barnes-Hut.
const SEED: u32 = 0;

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
        let mut run = ParticleMeshRun::from_frozen(topology, params)?;
        run.step_with(runner, workers, TICKS);
        planar_points(run.xs(), run.ys())
    }
}

/// One particle-mesh run, stepped by the caller: the stage steps it [`TICKS`] times, and
/// the bench times single ticks of it.
pub struct ParticleMeshRun {
    sim: Sim,
    mesh: Mesh,
    deltas: Vec<(f64, f64)>,
}

impl ParticleMeshRun {
    /// The run the stage makes, seeded on Barnes-Hut's golden spiral.
    pub fn from_frozen(topology: &Topology, params: &ForceParams) -> Result<Self, SessionError> {
        let params = LiveParams::from(*params);
        params.validate_finite()?;
        let sim = Sim::new(topology, params, SEED);
        let mesh = Mesh::new(sim.rows());
        Ok(Self {
            sim,
            mesh,
            deltas: Vec::new(),
        })
    }

    /// `ticks` ticks with the gathers divided by `runner` over `workers` workers.
    pub fn step_with(&mut self, runner: &impl Runner, workers: u32, ticks: u32) {
        for _ in 0..ticks {
            let mut how = How {
                runner,
                workers,
                deltas: &mut self.deltas,
                split: Split::None,
            };
            tick(&mut self.sim, &mut self.mesh, &mut how);
        }
    }

    /// The cooling schedule's current `alpha`.
    pub fn alpha(&self) -> f64 {
        self.sim.alpha
    }

    /// Every node's `x`, in dense node order.
    pub fn xs(&self) -> &[f64] {
        &self.sim.x
    }

    /// Every node's `y`, in dense node order.
    pub fn ys(&self) -> &[f64] {
        &self.sim.y
    }
}

/// Barnes-Hut's tick order with the mesh passes in place of the tree passes: decay, link,
/// many-body, center, collide, integrate.
fn tick<R: Runner>(sim: &mut Sim, mesh: &mut Mesh, how: &mut How<'_, R>) {
    sim.alpha += (sim.alpha_target - sim.alpha) * sim.params.alpha_decay;
    let split = how.split.splits(Split::Link);
    link::apply_with(sim, how.runner, how.workers, how.deltas, split);
    charge::apply(sim, mesh, how);
    sim.center();
    let collided = collide::apply(sim, &mut mesh.grid, how);
    let gathered = Gathered {
        deltas: how.deltas,
        slot: &mesh.grid.slot,
        split: how.split.splits(Split::Collide),
    };
    motion::integrate(sim, collided.then_some(gathered), (how.runner, how.workers));
    sim.tick_no += 1;
}
