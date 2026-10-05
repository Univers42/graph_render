//! The mesh a run owns: the FFT plan, the two `P × P` buffers the convolution runs in, the
//! kernel spectrum, and the collide grid whose node order the deposit reuses.
//!
//! One tick's field is: bound the nodes and place the frame, deposit every node's unit charge with CIC weights,
//! refresh the kernel if the rung moved, transform, multiply by the kernel, transform back,
//! the deposit and every transform pass split across the run's workers. The field then sits in rows
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
use super::deposit::{Deposit, Rows, Scaled, Stencils, weights};
use super::fft::{C, Fft, MAX_SIDE, Plan};
use super::frame::{self, Bounds, Frame};
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
    let root = libm::ceil(f64::sqrt(f64::from(n))) as usize;
    root.next_power_of_two().clamp(128, MAX_SIDE)
}

/// The seven words of a placed frame, as `u32`/`f64`: a rung is an `i32` on the way out and
/// a `u32` on the wire, and the origin is two columns rather than a tuple.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(in crate::layout::force) struct PlacedFrame {
    pub(in crate::layout::force) side: u32,
    pub(in crate::layout::force) step: i32,
    pub(in crate::layout::force) h: f64,
    pub(in crate::layout::force) origin_x: f64,
    pub(in crate::layout::force) origin_y: f64,
    pub(in crate::layout::force) cells: u32,
    pub(in crate::layout::force) reach: u32,
}

/// What a mesh's own solve produced for one tick: its frame and the two tables the
/// convolution runs on. The probe's public type inlines these seven frame words, so this
/// stays crate-internal.
#[derive(Debug, Clone)]
pub(in crate::layout::force) struct Solved {
    pub(in crate::layout::force) side: u32,
    pub(in crate::layout::force) step: i32,
    pub(in crate::layout::force) h: f64,
    pub(in crate::layout::force) origin_x: f64,
    pub(in crate::layout::force) origin_y: f64,
    pub(in crate::layout::force) cells: u32,
    pub(in crate::layout::force) reach: u32,
    pub(in crate::layout::force) twiddle_re: Vec<f64>,
    pub(in crate::layout::force) twiddle_im: Vec<f64>,
    pub(in crate::layout::force) spectrum_re: Vec<f64>,
    pub(in crate::layout::force) spectrum_im: Vec<f64>,
}

/// A run of complex samples as the two `f64` columns a wire carries.
fn split(values: &[C]) -> (Vec<f64>, Vec<f64>) {
    values.iter().map(C::parts).unzip()
}

pub(in crate::layout::force) struct Mesh {
    plan: Plan,
    density: Vec<C>,
    spectrum: Vec<C>,
    kernel: Kernel,
    frame: Option<Frame>,
    /// Each sorted slot's position in cell units this tick: the deposit splits it, then the
    /// field read splits it again, and neither reads a position.
    at: Vec<Scaled>,
    /// `at` grouped by row, the deposit's index.
    rows: Rows,
    /// The bounds fold's per-block boxes.
    blocks: Vec<Bounds>,
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
            at: vec![(0.0, 0.0); n as usize],
            rows: Rows::new(side, n),
            blocks: Vec::with_capacity(n.div_ceil(frame::BLOCK) as usize),
            grid: Grid::new(n),
        }
    }

    /// This mesh over `n` nodes, in place: every buffer a tick overwrites is resized and
    /// every buffer keyed on the side is rebuilt only when the side moves, so a batch of
    /// ten thousand rows costs the batch and not the whole mesh.
    ///
    /// A [`Mesh::new`] mesh is this one with every kept field reset, so each is either
    /// fully overwritten before the next tick reads it, or validly keyed — which is what
    /// makes the grown mesh the carried one, field for field:
    ///
    /// | Field | Kept when the side is unchanged because |
    /// |---|---|
    /// | `plan` | `Plan::new` is a pure function of the side, so the swaps and twiddles it holds are the ones a new plan would build |
    /// | `density`, `spectrum` | every FFT pass has `side²` outputs and `Runner::run` clears each range before the kernel writes it, and only rows `0..cells` are read, so no cell of the previous tick survives |
    /// | `kernel` | [`Kernel::refresh`] is keyed on the rung, the reach and the law, and a key hit means the next solve would resample and retransform the same `frame` — so the spectrum it holds is the one it would have built |
    /// | `blocks` | `frame::bounds` resizes it to `n / BLOCK` and writes every box it folds |
    /// | `frame` | never kept: set to `None` so a field read between the growth and the next tick is `(0, 0)`, as a fresh mesh's is |
    ///
    /// The three whose *length* is the node count — `at`, [`Rows`] and [`Grid`] — are
    /// resized rather than kept, and the two grid columns the tick's charge reads before
    /// that tick's collide rebuilds them are put back to the identity permutation a
    /// fresh grid holds ([`Grid::grow`] says which and why).
    pub(in crate::layout::force) fn grow(&mut self, n: u32) {
        let side = side_for(n);
        if side != self.plan.side() {
            self.plan = Plan::new(side);
            self.density = vec![C::default(); side * side];
            self.spectrum = vec![C::default(); side * side];
            // `built_for` does not name the side, so a kernel kept across a wider mesh
            // would hand a `side²`-long transform a shorter spectrum.
            self.kernel = Kernel::new(side);
        }
        self.frame = None;
        self.at.resize(n as usize, (0.0, 0.0));
        self.rows.grow(side, n);
        self.grid.grow(n);
    }

    /// The side this mesh's plan transforms, for the test that a growth crossed a
    /// [`side_for`] boundary.
    #[cfg(test)]
    pub(in crate::layout::force) fn side(&self) -> usize {
        self.plan.side()
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
        let xy = (&sim.x[..], &sim.y[..]);
        let found = frame::bounds(xy, runner, workers, &mut self.blocks);
        self.frame = found.and_then(|b| frame::place(b, side, f64::sqrt(law.dmax2)));
        let Some(frame) = self.frame.filter(|_| sim.x.len() > 1 && law.dmax2 > 0.0) else {
            return false;
        };
        self.deposit(&frame, xy, runner, workers);
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

    /// Unit charge per finite node into rows `0..cells` of `density`, the only rows the
    /// forward transform reads. The slots go in the collide grid's order, so consecutive
    /// nodes write neighbouring cells.
    fn deposit<R: Runner>(
        &mut self,
        frame: &Frame,
        xy: (&[f64], &[f64]),
        runner: &R,
        workers: u32,
    ) {
        let stencils = Stencils {
            frame,
            side: self.plan.side(),
            order: &self.grid.order,
            xy,
        };
        runner.run(&stencils, workers, &mut self.at);
        self.rows.sort(&self.at, frame);
        let deposit = Deposit {
            stencils: &stencils,
            at: &self.at,
            rows: &self.rows,
        };
        runner.run(&deposit, workers, &mut self.density);
    }

    /// The field at sorted slot `k`, read with the CIC weights its deposit used; zero for a
    /// non-finite node. Valid after a [`Mesh::solve`] that returned `true`.
    pub(super) fn field_of(&self, k: usize) -> (f64, f64) {
        let frame = self.frame.as_ref();
        frame
            .and_then(|f| frame::split(f, self.at[k]))
            .map_or((0.0, 0.0), |s| self.read(s))
    }

    /// The frame, the plan's forward twiddles and the kernel spectrum this tick's solve
    /// produced, as `f64` columns, or `None` before a [`Mesh::solve`] that found a field.
    ///
    /// The caller owns the columns: nothing here narrows, rescales or reorders, so the
    /// spectrum keeps the `1/P²` [`Kernel::refresh`] folded in and the table keeps the
    /// stage-`half` layout the transform reads it in.
    pub(in crate::layout::force) fn solution(&self) -> Option<Solved> {
        let frame = self.frame.as_ref()?;
        let (twiddle_re, twiddle_im) = split(self.plan.twiddles());
        let (spectrum_re, spectrum_im) = split(&self.kernel.spectrum);
        Some(Solved {
            side: self.plan.side() as u32,
            step: frame.step,
            h: frame.h,
            origin_x: frame.origin.0,
            origin_y: frame.origin.1,
            cells: frame.cells as u32,
            reach: frame.reach as u32,
            twiddle_re,
            twiddle_im,
            spectrum_re,
            spectrum_im,
        })
    }

    /// The field at `p`, read with the CIC weights the deposit used; zero for a non-finite
    /// position or before any field was solved.
    #[cfg(test)]
    pub(super) fn field_at(&self, p: (f64, f64)) -> (f64, f64) {
        let stencil = self.frame.and_then(|f| frame::stencil(&f, p));
        stencil.map_or((0.0, 0.0), |s| self.read(s))
    }

    /// The frame [`frame::place_over`] places over these positions at this side and reach,
    /// as the seven words the probe inlines, so the probe's own test compares against the
    /// mesh's own placement rather than against itself.
    #[cfg(test)]
    pub(in crate::layout::force) fn placed_over(
        &self,
        xy: (&[f64], &[f64]),
        side: u32,
        dmax: f64,
    ) -> Option<PlacedFrame> {
        frame::place_over(xy, side as usize, dmax).map(|f| PlacedFrame {
            side,
            step: f.step,
            h: f.h,
            origin_x: f.origin.0,
            origin_y: f.origin.1,
            cells: f.cells as u32,
            reach: f.reach as u32,
        })
    }

    fn read(&self, ((cx, cy), (fx, fy)): ((usize, usize), (f64, f64))) -> (f64, f64) {
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
