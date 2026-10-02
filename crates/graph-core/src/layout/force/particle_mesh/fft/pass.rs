//! The 2D transform as two range passes a [`Runner`] can split across workers. Output row
//! `r` of a pass is one line transform of input row `r` ([`Read::Rows`]) or of input column
//! `r` ([`Read::Columns`], the transpose fused into the load), so rows then columns is the
//! 2D transform, transposed. Every line sees the inputs the old rows / transpose / rows
//! sequence gave it, in the same order, so the bytes are that sequence's; `tests.rs` keeps
//! it as the reference and compares bit for bit.
//!
//! Two prunings, both exact. A forward transform skips the input rows from `live` on, which
//! the deposit does not even store: they are +0, and a line of +0 is +0 (every butterfly is
//! `+0 ± t` with `t` a zero, which rounds to +0). An inverse skips the output rows from
//! `live` on, which no node reads.
//!
//! Caveat: the [`Runner`] clears and refills `out` with `Default` before a pass, a serial
//! write of `side²` samples (16 MiB at side 1024) that the threads do not share.

use super::{C, MAX_SIDE, Plan};
use crate::exec::{Runner, StepRange};
use std::ops::Range;

/// Where output row `r` of a pass reads its line.
#[derive(Clone, Copy)]
enum Read<'a> {
    /// Input row `r`.
    Rows,
    /// Input row `r`, each sample multiplied by the same index of this buffer first.
    RowsTimes(&'a [C]),
    /// Input column `r`.
    Columns,
}

/// One pass: `side²` outputs, row by row.
struct Pass<'a> {
    plan: &'a Plan,
    src: &'a [C],
    read: Read<'a>,
    inverse: bool,
    /// Output rows from here on are zero, not transformed.
    live: usize,
}

impl Pass<'_> {
    /// Output row `r` into `dst`, one line long.
    fn line_into(&self, r: usize, dst: &mut [C]) {
        if r >= self.live {
            dst.fill(C::default());
            return;
        }
        let side = self.plan.side();
        let row = &self.src[r * side..][..side];
        match self.read {
            Read::Rows => dst.copy_from_slice(row),
            Read::RowsTimes(gain) => {
                for ((d, &s), &g) in dst.iter_mut().zip(row).zip(&gain[r * side..][..side]) {
                    *d = s * g;
                }
            }
            Read::Columns => {
                for (d, &s) in dst.iter_mut().zip(self.src[r..].iter().step_by(side)) {
                    *d = s;
                }
            }
        }
        self.plan.line(dst, self.inverse);
    }
}

impl StepRange for Pass<'_> {
    type Out = C;

    fn len(&self) -> u32 {
        (self.plan.side() * self.plan.side()) as u32
    }

    // A range may start or end inside a row: that row is transformed whole on the stack
    // and only the range's part copied out, so at most two lines per range run twice.
    fn step_range(&self, range: Range<u32>, out: &mut [C]) {
        let side = self.plan.side();
        let (mut at, end) = (range.start as usize, range.end as usize);
        let mut done = 0;
        while at < end {
            let (row, from) = (at / side, at % side);
            let to = (end - row * side).min(side);
            let span = &mut out[done..done + to - from];
            if to - from == side {
                self.line_into(row, span);
            } else {
                let mut line = [C::default(); MAX_SIDE];
                self.line_into(row, &mut line[..side]);
                span.copy_from_slice(&line[from..to]);
            }
            done += to - from;
            at = row * side + to;
        }
    }
}

/// A plan and who runs its passes.
pub(in crate::layout::force::particle_mesh) struct Fft<'a, R> {
    pub(in crate::layout::force::particle_mesh) plan: &'a Plan,
    pub(in crate::layout::force::particle_mesh) runner: &'a R,
    pub(in crate::layout::force::particle_mesh) workers: u32,
}

impl<R: Runner> Fft<'_, R> {
    /// The forward 2D transform of `a`, whose rows from `live` on are +0 and need not be
    /// stored, into `a`, transposed: `a[kx * side + ky]`. `b` is scratch.
    pub(in crate::layout::force::particle_mesh) fn forward(
        &self,
        (a, b): (&mut Vec<C>, &mut Vec<C>),
        live: usize,
    ) {
        self.pass(a, Read::Rows, (false, live), b);
        self.pass(b, Read::Columns, (false, self.plan.side()), a);
    }

    /// The inverse 2D transform of `a * gain`, both transposed spectra, into `a`, rows
    /// `0..live` only; the rest is zero. `b` is scratch.
    pub(in crate::layout::force::particle_mesh) fn inverse(
        &self,
        (a, b): (&mut Vec<C>, &mut Vec<C>),
        gain: &[C],
        live: usize,
    ) {
        self.pass(a, Read::RowsTimes(gain), (true, self.plan.side()), b);
        self.pass(b, Read::Columns, (true, live), a);
    }

    fn pass(&self, src: &[C], read: Read<'_>, (inverse, live): (bool, usize), out: &mut Vec<C>) {
        let pass = Pass {
            plan: self.plan,
            src,
            read,
            inverse,
            live,
        };
        self.runner.run(&pass, self.workers, out);
    }
}
