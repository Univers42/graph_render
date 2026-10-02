//! The mesh a run owns: the FFT plan, the two `P × P` buffers the convolution runs in, the
//! kernel spectrum, and the collide grid whose node order the deposit reuses.
//!
//! One tick's field is: place the frame, deposit every node's unit charge with CIC weights,
//! refresh the kernel if the rung moved, transform, multiply by the kernel, transform back,
//! every transform pass split across the run's workers. The field then sits in rows
//! `0..cells` of `density`, `Ex` real and `Ey` imaginary, and a node reads
//! it with the same four CIC weights it deposited with.
//!
//! Caveat: the field is the law convolved at cell resolution, so it is exact at range and
//! smooth below about two cells: two nodes closer than `2h` repel less than the direct sum
//! says, and two nodes inside one cell barely at all. Collide and link dominate at that
//! range, and the stress gate is what grades the result; `h` per size is in
//! `docs/measurements/perf-p2-pm.md`. Coincident nodes get no jiggle here: they separate
//! through collide's.

use super::collide::Grid;
use super::fft::{C, Fft, MAX_SIDE, Plan};
use super::frame::{self, Frame};
use super::kernel::{Kernel, Law};
use crate::exec::Runner;
use crate::layout::force::barnes_hut::sim::Sim;

/// The mesh side for `n` nodes: `ceil(sqrt n)` rounded up to a power of two, held to
/// `128..=MAX_SIDE`.
///
/// Caveat: a fixed side trades resolution for time. At 1M nodes the side caps at 1024
/// cells for the whole layout, so `h` grows with the span; the floor of 128 keeps a small
/// graph's cells well under its link distance.
pub(super) fn side_for(n: u32) -> usize {
    let root = libm::ceil(libm::sqrt(f64::from(n))) as usize;
    root.next_power_of_two().clamp(128, MAX_SIDE)
}

pub(in crate::layout::force) struct Mesh {
    plan: Plan,
    density: Vec<C>,
    spectrum: Vec<C>,
    kernel: Kernel,
    frame: Option<Frame>,
    pub(super) grid: Grid,
}

impl Mesh {
    pub(in crate::layout::force) fn new(n: u32) -> Mesh {
        let side = side_for(n);
        Mesh {
            plan: Plan::new(side),
            density: vec![C::default(); side * side],
            spectrum: vec![C::default(); side * side],
            kernel: Kernel::new(side),
            frame: None,
            grid: Grid::new(n),
        }
    }

    /// This tick's field over `sim`'s positions, its transforms run by `runner` on
    /// `workers`. `false` when there is none to read: fewer than two nodes, a zero
    /// `distanceMax`, or no finite position.
    pub(super) fn solve<R: Runner>(&mut self, sim: &Sim, runner: &R, workers: u32) -> bool {
        let p = &sim.params;
        let law = Law {
            dmin2: p.distance_min * p.distance_min,
            dmax2: p.distance_max * p.distance_max,
        };
        let side = self.plan.side();
        self.frame = frame::place(&sim.x, &sim.y, side, libm::sqrt(law.dmax2));
        let Some(frame) = self.frame.filter(|_| sim.x.len() > 1 && law.dmax2 > 0.0) else {
            return false;
        };
        self.deposit(&frame, (&sim.x, &sim.y));
        let fft = Fft {
            plan: &self.plan,
            runner,
            workers,
        };
        self.kernel.refresh(&fft, (&frame, law), &mut self.spectrum);
        let buffers = (&mut self.density, &mut self.spectrum);
        fft.forward(buffers, frame.cells);
        let buffers = (&mut self.density, &mut self.spectrum);
        fft.inverse(buffers, &self.kernel.spectrum, frame.cells);
        true
    }

    /// Unit charge per finite node, in the collide grid's order so consecutive nodes write
    /// neighbouring cells.
    fn deposit(&mut self, frame: &Frame, (x, y): (&[f64], &[f64])) {
        self.density.fill(C::default());
        let side = self.plan.side();
        for &i in &self.grid.order {
            let i = i as usize;
            let Some(((cx, cy), (fx, fy))) = frame::stencil(frame, (x[i], y[i])) else {
                continue;
            };
            let at = cy * side + cx;
            for (cell, w) in [at, at + 1, at + side, at + side + 1]
                .into_iter()
                .zip(weights(fx, fy))
            {
                self.density[cell].re += w;
            }
        }
    }

    /// The field at `p`, read with the CIC weights the deposit used; zero for a non-finite
    /// position or before any field was solved.
    pub(super) fn field_at(&self, p: (f64, f64)) -> (f64, f64) {
        let Some(((cx, cy), (fx, fy))) = self.frame.and_then(|f| frame::stencil(&f, p)) else {
            return (0.0, 0.0);
        };
        let side = self.plan.side();
        let at = cy * side + cx;
        let mut e = (0.0, 0.0);
        for (cell, w) in [at, at + 1, at + side, at + side + 1]
            .into_iter()
            .zip(weights(fx, fy))
        {
            e.0 += self.density[cell].re * w;
            e.1 += self.density[cell].im * w;
        }
        e
    }
}

/// The four CIC weights, in the cell order `(x, y), (x+1, y), (x, y+1), (x+1, y+1)`.
fn weights(fx: f64, fy: f64) -> [f64; 4] {
    [
        (1.0 - fx) * (1.0 - fy),
        fx * (1.0 - fy),
        (1.0 - fx) * fy,
        fx * fy,
    ]
}
