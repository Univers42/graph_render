//! A radix-2 complex FFT over `f64`, the one transform the particle-mesh charge pass needs:
//! a plan per mesh side, lines transformed in place, and a 2D transform written as rows, a
//! blocked transpose, then rows again.
//!
//! Written here rather than imported because graph-core's dependency list is closed (libm,
//! indexmap, petgraph). It is deterministic in the D1–D10 sense: the twiddles come from
//! libm, the butterfly order is fixed by the plan, and every product is a plain `*` that
//! rustc never contracts into an FMA, so native and wasm32 compute the same bits.
//!
//! The inverse is unnormalised. The caller folds the `1/P²` into the kernel spectrum once,
//! which is exact because `P` is a power of two.

use std::ops::{Add, Mul, Sub};

/// One complex sample.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct C {
    pub(super) re: f64,
    pub(super) im: f64,
}

impl Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }
}

impl Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C {
            re: self.re - o.re,
            im: self.im - o.im,
        }
    }
}

impl Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
}

/// The bit-reversal permutation and both twiddle tables for lines of `side` samples.
pub(super) struct Plan {
    side: usize,
    /// The bit-reversal permutation as the swaps it takes, `i < j` only.
    swaps: Vec<(u32, u32)>,
    /// Stage `half`'s twiddles `w^(j * side / (2 * half))`, `j < half`, at
    /// `half..2 * half`, so a stage reads its twiddles as one contiguous run.
    forward: Vec<C>,
    inverse: Vec<C>,
}

impl Plan {
    /// `side` must be a power of two, at least 2.
    pub(super) fn new(side: usize) -> Plan {
        assert!(side >= 2 && side.is_power_of_two(), "fft side {side}");
        let bits = side.trailing_zeros();
        let swaps = (0..side as u32)
            .map(|i| (i, i.reverse_bits() >> (32 - bits)))
            .filter(|&(i, j)| i < j)
            .collect();
        let root: Vec<C> = (0..side / 2)
            .map(|k| {
                let angle = -core::f64::consts::TAU * k as f64 / side as f64;
                C {
                    re: libm::cos(angle),
                    im: libm::sin(angle),
                }
            })
            .collect();
        let forward: Vec<C> = (0..side)
            .map(|k| {
                let half = 1 << k.max(1).ilog2();
                root[(k % half) * (side / (2 * half))]
            })
            .collect();
        let inverse = forward
            .iter()
            .map(|w| C {
                re: w.re,
                im: -w.im,
            })
            .collect();
        Plan {
            side,
            swaps,
            forward,
            inverse,
        }
    }

    pub(super) fn side(&self) -> usize {
        self.side
    }

    /// One line of `side` samples, in place. The first stage's twiddle is 1, so it is a
    /// plain sum and difference.
    pub(super) fn line(&self, a: &mut [C], inverse: bool) {
        for &(i, j) in &self.swaps {
            a.swap(i as usize, j as usize);
        }
        for [u, v] in a.as_chunks_mut::<2>().0 {
            (*u, *v) = (*u + *v, *u - *v);
        }
        let twiddle = if inverse {
            &self.inverse
        } else {
            &self.forward
        };
        let mut half = 2;
        while half < self.side {
            let stage = &twiddle[half..2 * half];
            for block in a.chunks_exact_mut(2 * half) {
                let (lo, hi) = block.split_at_mut(half);
                for ((u, v), &w) in lo.iter_mut().zip(hi.iter_mut()).zip(stage) {
                    let t = *v * w;
                    (*u, *v) = (*u + t, *u - t);
                }
            }
            half *= 2;
        }
    }

    /// Every row of a `side × side` buffer, in place.
    pub(super) fn rows(&self, buf: &mut [C], inverse: bool) {
        for row in buf.chunks_exact_mut(self.side) {
            self.line(row, inverse);
        }
    }

    /// The 2D transform of `src` into `dst`, **transposed**: `dst[kx * side + ky]` is the
    /// coefficient `src` row-major would put at `[ky * side + kx]`. Transforming twice
    /// lands back in the original layout, which is how the charge pass uses it: forward
    /// into the spectrum, multiply, inverse back. `src` is overwritten with scratch.
    pub(super) fn fft2(&self, src: &mut [C], dst: &mut [C], inverse: bool) {
        self.rows(src, inverse);
        transpose(src, dst, self.side);
        self.rows(dst, inverse);
    }
}

/// Square tile edge for [`transpose`]: 32 × 16 bytes is two 512-byte rows a tile, so a
/// tile's source and destination rows both stay in L1.
const TILE: usize = 32;

/// `dst[x * side + y] = src[y * side + x]`, tile by tile.
pub(super) fn transpose(src: &[C], dst: &mut [C], side: usize) {
    for ty in (0..side).step_by(TILE) {
        for tx in (0..side).step_by(TILE) {
            tile(src, dst, side, (tx, ty));
        }
    }
}

fn tile(src: &[C], dst: &mut [C], side: usize, (tx, ty): (usize, usize)) {
    for y in ty..(ty + TILE).min(side) {
        for x in tx..(tx + TILE).min(side) {
            dst[x * side + y] = src[y * side + x];
        }
    }
}

#[cfg(test)]
mod tests;
