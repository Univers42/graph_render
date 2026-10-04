//! The repulsion: `_pair_force` as `_repulsion_direct` calls it (`simulation.py:72-92`,
//! `:527-530`).
//!
//! ```text
//! s2    = einsum("ij,ij->i", sources, sources)             f32, sequential
//! d2    = einsum("ij,ij->i", t, t)[:, None] + s2[None, :]   f32
//! d2   -= 2.0 * (t @ sources.T)                            f32   <- BLAS sgemm, k = 3
//! d2    = maximum(d2, 0.0); d2 += soften                    f32
//! coeff = (repulsion * t_mass[:, None]) * s_mass[None, :]   f64  <- see the module doc
//! coeff /= d2                                              f64
//! coeff[i, i] = 0.0
//! out[lo:hi] = t * coeff.sum(axis=1)[:, None] - coeff @ sources   f64, narrowed to f32
//! ```
//!
//! **Two reductions here are not reproducible, and are named as the row's residual
//! causes.** `t @ sources.T` is a BLAS `sgemm` with an inner dimension of three: measured,
//! it is exactly an FMA chain (0 of 5929 cells differ from `acc = fma(a_k, b_k, acc)`;
//! 2275 of 5929 differ from a sequential `f32` dot), and this port uses the sequential
//! `f32` dot instead — an FMA is not a portable primitive under D1, and emulating one
//! through `f64` would be a double rounding dressed up as a single one. `coeff @ sources`
//! is a BLAS `dgemm` over `n`; this port sums sequentially. Measured effect of both, over
//! all 24 conformance fixtures: **none** — the finished layouts are bitwise identical
//! either way, because both products are rounded to `f32` on store (`simulation.py:74`) and
//! an `f64` ulp dies in that rounding. `docs/measurements/sg-fa2-forcesim.md` has the
//! probe and the numbers.

use super::reduce::{dot3_f32, dot3_pair_f32, pairwise_f64};

/// The reference's `CHUNK` (`simulation.py:20`): the strip of targets whose coefficient
/// block is materialised at once, so the transient is `CHUNK * n * 8` bytes rather than
/// `n * n * 8`.
const CHUNK: usize = 2048;

/// One node's repulsion against every node, through the reference's `f64` coefficient
/// block.
///
/// A struct rather than four parameters threaded through three helpers: `run` needs all
/// four and so does `fill`, so the list would be carried by every call site for nothing.
pub(super) struct Direct<'a> {
    pos: &'a [f32],
    mass: &'a [f32],
    /// `self.repulsion`: an `np.float64` scalar, which is the whole reason this term is
    /// `f64` and not `f32` (see the module doc's dtype trace).
    repulsion: f64,
    /// `DTYPE((0.01 * self.k) ** 2)` — the squared softening, already narrowed.
    soften: f32,
}

impl<'a> Direct<'a> {
    pub(super) fn new(pos: &'a [f32], mass: &'a [f32], repulsion: f64, soften: f32) -> Self {
        Self {
            pos,
            mass,
            repulsion,
            soften,
        }
    }

    /// The whole `(n, 3)` field, chunked over the targets as the reference chunks them.
    pub(super) fn run(&self) -> Vec<f32> {
        let n = self.mass.len();
        let mut out = vec![0.0f32; 3 * n];
        // `s2` runs over *every* source and is computed once, before the chunk loop
        // (`simulation.py:75`). For a target's own row `einsum(t, t)` is the same
        // reduction, so this one array serves both.
        let s2: Vec<f32> = (0..n).map(|j| dot3_f32(self.row(j))).collect();
        let mut block = vec![0.0f64; CHUNK.min(n) * n];
        for lo in (0..n).step_by(CHUNK) {
            let hi = (lo + CHUNK).min(n);
            self.fill(&mut block[..(hi - lo) * n], &s2, lo);
            for i in lo..hi {
                let row = &block[(i - lo) * n..(i - lo + 1) * n];
                self.write(row, i, &mut out);
            }
        }
        out
    }

    /// `coeff` for targets `lo..hi`: the `f64` `(hi - lo) * n` block, diagonal zeroed.
    ///
    /// `coeff` is exactly `rows * n` long, so `chunks_exact_mut(n)` walks one row per
    /// target and `lo + i` is the global index of the row being filled.
    fn fill(&self, coeff: &mut [f64], s2: &[f32], lo: usize) {
        let n = self.mass.len();
        for (i, slot) in coeff.chunks_exact_mut(n).enumerate() {
            let target = lo + i;
            let row = self.row(target);
            let own = dot3_f32(row);
            for (j, coefficient) in slot.iter_mut().enumerate() {
                let mut d2 = own + s2[j];
                d2 -= 2.0f32 * dot3_pair_f32(row, self.row(j));
                if d2 < 0.0 {
                    d2 = 0.0;
                }
                d2 += self.soften;
                *coefficient =
                    self.repulsion * f64::from(self.mass[target]) * f64::from(self.mass[j])
                        / f64::from(d2);
            }
            // `skip_self=True` zeroes the diagonal *after* the division
            // (`simulation.py:87-89`), so the softening above still applied to it.
            slot[target] = 0.0;
        }
    }

    /// `t * coeff.sum(axis=1)[:, None] - coeff @ sources`, in `f64`, narrowed to `f32`.
    ///
    /// `coeff.sum(axis=1)` is numpy's pairwise `f64`; `coeff @ sources` is the `dgemm` this
    /// port replaces with a sequential `f64` dot (see the module doc).
    fn write(&self, row: &[f64], i: usize, out: &mut [f32]) {
        let total = pairwise_f64(row);
        for c in 0..3 {
            let mut product = 0.0f64;
            for (j, &coefficient) in row.iter().enumerate() {
                product += coefficient * f64::from(self.pos[3 * j + c]);
            }
            out[3 * i + c] = (f64::from(self.pos[3 * i + c]) * total - product) as f32;
        }
    }

    /// Node `j`'s three coordinates, as a slice of the flat `f32` state (D10: gather).
    fn row(&self, j: usize) -> &[f32] {
        &self.pos[3 * j..][..3]
    }
}
