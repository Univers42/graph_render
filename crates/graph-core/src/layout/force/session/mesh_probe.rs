//! The mesh probe: the particle mesh at one session state, as the columns a device needs
//! to run the same tick — the collapsed adjacency, the frame, the twiddles and the kernel
//! spectrum, and what each of the mesh's three force passes adds to the velocities from
//! rest.
//!
//! # Why this is not a second implementation
//!
//! It computes no force law and no spectrum of its own. Every column is read out of the
//! mesh's own solve on a *copy* of the session, exactly as
//! [`charge_deltas`](ForceSession::charge_deltas) does for the many-body pass alone: the
//! copy's velocities start at zero and `alpha` at 1, so a pass's increment into `vx`/`vy`
//! is the force, to the last bit (`session/fidelity.rs`). The twiddles are the plan's and
//! the spectrum is the kernel's, both built with `libm` on the CPU and read out whole.
//!
//! Caveat, and it travels with every number this hands back: **the copy is at tick 0**
//! whatever the session's own tick is (`session/fidelity.rs:26-29`). Collide reads
//! `tick_no` for its coincidence jiggle, so a *settled* state's collide column is the
//! collide pass at tick 0 — the state and the pass are two different clocks.
//!
//! Caveat: the frame is recomputed from the positions **every tick**
//! (`particle_mesh/mesh.rs:119-125`), so a start state and a settled state have different
//! frames, different `h` and different kernel spectra. One probe is one state.

use super::ForceSession;
use super::fidelity::{PROBE_WORKERS, charge_probe_on};
use crate::exec::Serial;
use crate::layout::force::Split;
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::layout::force::particle_mesh::{self, Mesh, MeshPass};

/// The CPU mesh at one session state: its adjacency, frame, twiddles, kernel spectrum, and
/// what each force pass adds to the velocities from rest.
///
/// Every length is derivable from `n = lo.len()`'s sibling columns and `side`, and every
/// column is the mesh's own `f64`: the whole point is that the device narrows and this
/// reference does not.
#[derive(Debug, Clone)]
pub struct MeshProbe {
    /// Each simple edge's lower-index endpoint, in edge order.
    pub lo: Vec<u32>,
    /// Each simple edge's higher-index endpoint, in the same order.
    pub hi: Vec<u32>,
    /// The surviving raw edge's strength per simple edge (`layout/force/mod.rs:64-69`).
    pub strength: Vec<f64>,
    /// The transform side `P`, a power of two in `128..=1024`.
    pub side: u32,
    /// The frame's rung; `h = 2^(step/4)` (`particle_mesh/frame.rs:74-89`).
    pub step: i32,
    /// The cell size in position units.
    pub h: f64,
    /// The frame's left edge, snapped down to a multiple of `h`.
    pub origin_x: f64,
    /// The frame's bottom edge, snapped the same way.
    pub origin_y: f64,
    /// The frame's cells per axis, in cells.
    pub cells: u32,
    /// How far the kernel reaches, in cells.
    pub reach: u32,
    /// The plan's forward twiddles, `side` of them, real part.
    pub twiddle_re: Vec<f64>,
    /// …and imaginary part. Stage `half` reads `twiddle[half..2*half]`.
    pub twiddle_im: Vec<f64>,
    /// The kernel spectrum in the transposed layout `a[kx * side + ky]`, real part,
    /// already pre-scaled by `1/P²` (`particle_mesh/kernel.rs:66-76`).
    pub spectrum_re: Vec<f64>,
    /// …and imaginary part.
    pub spectrum_im: Vec<f64>,
    /// What the link pass adds to `vx`, per node in row order, from rest.
    pub link_dx: Vec<f64>,
    /// …and to `vy`.
    pub link_dy: Vec<f64>,
    /// What the many-body pass adds to `vx`.
    pub charge_dx: Vec<f64>,
    /// …and to `vy`.
    pub charge_dy: Vec<f64>,
    /// What the collide pass adds to `vx`, at the copy's tick 0.
    pub collide_dx: Vec<f64>,
    /// …and to `vy`.
    pub collide_dy: Vec<f64>,
}

/// One mesh pass run against its own copy: the mutated `Sim` and the `Mesh` it ran on,
/// which after a many-body pass holds the frame and the spectrum.
struct Run {
    sim: Sim,
    mesh: Mesh,
}

impl ForceSession {
    /// The CPU mesh at this session's state, or `None` when there is no field to solve:
    /// a session that does not tick the mesh at all, fewer than two nodes, a zero
    /// `distance_max`, or no finite position (`particle_mesh/mesh.rs:114-125`).
    ///
    /// Does not move the session: the positions, the velocities, the parameters, the pins
    /// and the tick number are exactly as they were on entry and on return.
    ///
    /// Caveat: every force column here is from a copy at **tick 0** and at `alpha` 1, so
    /// collide's column is not the collide pass this session would run next.
    pub fn mesh_probe(&self) -> Option<MeshProbe> {
        self.mesh.as_ref()?;
        let charge = self.run(MeshPass::Charge);
        let solved = charge.mesh.solution()?;
        let (link_dx, link_dy) = self.deltas(MeshPass::Link);
        let (collide_dx, collide_dy) = self.deltas(MeshPass::Collide);
        let graph = &self.sim.graph;
        Some(MeshProbe {
            lo: graph.lo.clone(),
            hi: graph.hi.clone(),
            strength: graph.strength.clone(),
            side: solved.side,
            step: solved.step,
            h: solved.h,
            origin_x: solved.origin_x,
            origin_y: solved.origin_y,
            cells: solved.cells,
            reach: solved.reach,
            twiddle_re: solved.twiddle_re,
            twiddle_im: solved.twiddle_im,
            spectrum_re: solved.spectrum_re,
            spectrum_im: solved.spectrum_im,
            link_dx,
            link_dy,
            charge_dx: charge.sim.vx,
            charge_dy: charge.sim.vy,
            collide_dx,
            collide_dy,
        })
    }

    /// The two velocity columns one pass adds, from a copy at rest.
    fn deltas(&self, pass: MeshPass) -> (Vec<f64>, Vec<f64>) {
        let run = self.run(pass);
        (run.sim.vx, run.sim.vy)
    }

    /// One pass against a fresh copy of this session, its own mesh, and a serial `How`
    /// with one worker — `charge_deltas`'s own schedule, so the columns are comparable.
    fn run(&self, pass: MeshPass) -> Run {
        let runner = Serial;
        let mut sim = charge_probe_on(self, 0.0);
        let mut mesh = Mesh::new(sim.rows());
        let mut deltas = Vec::with_capacity(sim.rows() as usize);
        let mut how = How {
            runner: &runner,
            workers: PROBE_WORKERS,
            deltas: &mut deltas,
            split: Split::None,
        };
        particle_mesh::pass(&mut sim, &mut mesh, &mut how, pass);
        Run { sim, mesh }
    }
}

#[cfg(test)]
#[path = "mesh_probe/tests.rs"]
mod tests;
