//! Graphviz's `drand48`, which is what `initLayout` reads every coordinate out of.
//!
//! **This is the reason the seed is part of the port.** `initLayout` writes two draws per
//! node into `double` coordinates (`stress.c:154-155`) and `checkStart` seeds the generator
//! from the `start` attribute (`neatoinit.c:989`), so for `neato` — unlike for `twopi` —
//! `-Gstart` decides the drawing. All 1000 differential fixtures draw differently at
//! `-Gstart` 1, 7 and 99 (`docs/measurements/p13-gv2-neato.md`).
//!
//! The generator is the POSIX `drand48`, a 48-bit linear congruential one:
//!
//! ```text
//! srand48(seed):  X = (seed << 16) | 0x330E
//! drand48():      X = (0x5DEECE66D * X + 0xB) mod 2^48
//!                 return X / 2^48
//! ```
//!
//! Reproduced in `u64` integer arithmetic with an explicit `mod 2^48`, which is exact: the
//! state is 48 bits, the multiplier is 49, and the product is under 2^97, so it fits in a
//! `u64` only because the mask is applied *after* the multiply — hence `wrapping_mul` and
//! an explicit `& MASK`, not a `u64` multiply that a reader would assume cannot overflow.
//! It cannot: `0x5DEECE66D * (2^48 - 1) < 2^97` is 97 bits, so the multiply *does* need the
//! masking step to stay in range, and `u64::wrapping_mul` is what keeps the shift-free
//! expression readable without a `u128` in the middle of a draw.
//!
//! The final division is by a power of two, so it is exact for every draw: `X < 2^48` is
//! representable in `f64` and the scaling loses no bit. The values are therefore the same
//! `f64`s the reference's `drand48` returns, native and wasm32 alike (D1), and no
//! transcendental is involved at all.

/// The generator's modulus, `2^48`, and the mask that applies it.
const MASK: u64 = (1 << 48) - 1;

/// The multiplier, `0x5DEECE66D` (`0x5DEECE66D` = 25214903917).
const MULTIPLIER: u64 = 0x0005_DEEC_E66D;

/// The increment, `0xB`.
const INCREMENT: u64 = 0xB;

/// The low 16 bits `srand48` folds into a seed's high bits: `(seed << 16) | 0x330E`.
const SEED_TAIL: u64 = 0x330E;

/// The scaling a draw is divided by, `2^48`, exactly.
const SCALE: f64 = 281_474_976_710_656.0;

/// `drand48` as `srand48` leaves it, after `checkStart` has read the `start` attribute.
pub(super) struct Drand48 {
    state: u64,
}

impl Drand48 {
    /// `srand48(seed)`: the state is the seed in the high 32 bits and `0x330E` in the low
    /// 16, so a seed of 1 starts from `0x1330E` rather than from 1.
    pub(super) fn seeded(seed: u32) -> Self {
        Self {
            state: (u64::from(seed) << 16 | SEED_TAIL) & MASK,
        }
    }

    /// One `drand48()`: advance the state, then scale it into `[0, 1)`.
    pub(super) fn next(&mut self) -> f64 {
        self.state = self.state.wrapping_mul(MULTIPLIER).wrapping_add(INCREMENT) & MASK;
        self.state as f64 / SCALE
    }
}

#[cfg(test)]
mod tests {
    use super::{Drand48, MASK, SCALE, SEED_TAIL};

    /// The state after `srand48` is `(seed << 16) | 0x330E`, which is what makes a seed of
    /// 1 draw `0.0416...` rather than something derived from 1 directly.
    #[test]
    fn the_seeded_state_is_the_seed_in_the_high_bits() {
        let mut one = Drand48::seeded(1);
        let mut direct = Drand48 { state: 0 };
        direct.state = (1 << 16 | SEED_TAIL) & MASK;
        assert_eq!(one.next(), direct.next());
    }

    /// Every draw is in `[0, 1)`, including the largest state the generator can reach: a
    /// `drand48` of exactly 1.0 would break the initial placement's own assumption.
    #[test]
    fn every_draw_is_below_one() {
        let mut rng = Drand48 { state: MASK };
        for _ in 0..64 {
            let draw = rng.next();
            assert!((0.0..1.0).contains(&draw), "{draw}");
        }
    }

    /// The state never leaves 48 bits, so `wrapping_mul` is a masking convenience rather
    /// than a source of a different answer: an unmasked multiply would have to agree here
    /// for the port to be right at all.
    #[test]
    fn the_state_stays_inside_forty_eight_bits() {
        let mut rng = Drand48::seeded(1);
        for _ in 0..1000 {
            rng.next();
            assert!(rng.state <= MASK, "{}", rng.state);
        }
    }

    /// The scale is `2^48`, so a draw is exact and no rounding can accumulate: a state of
    /// one maps to `2^-48`, not to the nearest `f64` below it.
    #[test]
    fn the_scale_is_exactly_two_to_the_forty_eight() {
        // `2^48` in every spelling, and exact in both directions: the reciprocal of a power
        // of two is representable, so the division in `next` costs no bits at all.
        assert_eq!(SCALE, 281_474_976_710_656.0);
        assert_eq!(SCALE, (1u64 << 48) as f64);
        assert_eq!(SCALE * (1.0 / SCALE), 1.0);
    }
}
