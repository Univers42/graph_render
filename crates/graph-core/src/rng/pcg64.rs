//! The generator `np.random.default_rng` draws from, ported so a *start position* can be
//! SciGraphs' rather than merely like it.
//!
//! Three separate facts, and getting any one of them wrong gives a stream that looks
//! plausible and is not the reference's:
//!
//! - **The chain.** `default_rng(seed)` is `PCG64(SeedSequence(seed))`; `_pcg64.pyx:132`
//!   takes `generate_state(4, np.uint64)` — four hashed words — and hands them straight to
//!   `pcg64_set_seed`. The words are not the seed. See
//!   [`pcg64::seed_sequence`](self::seed_sequence).
//! - **The seeding.** `pcg64_set_seed` (`pcg64.c:148`) packs `w0:w1` as `initstate` and
//!   `w2:w3` as `initseq`, then `pcg_setseq_128_srandom_r` (`pcg64.h`) does
//!   `state = 0`, `inc = (initseq << 1) | 1`, one step, `state += initstate`, one step.
//! - **The step and the output.** `pcg64_next64` advances the 128-bit LCG
//!   (`state = state * MULT + inc`) and *then* permutes:
//!   `rotr64(state.high ^ state.low, state.high >> 58)`. The `f64` is the top 53 bits
//!   times `2^-53` (`uint64_to_double`, `distributions.c`).
//!
//! **The state is integer-only** — xor, shift, `wrapping_mul`, `u128` add — so the stream
//! is bit-identical on every target, native and wasm32 alike (D1, D2, D10). `u128`
//! arithmetic is exact on wasm32 too: it is software, not a widened float.

mod seed_sequence;

#[cfg(test)]
mod tests;

use seed_sequence::SeedSequence;

/// PCG-XSL-RR 128/64, the generator `np.random.default_rng` names `PCG64`.
#[derive(Debug, Clone)]
pub(crate) struct Pcg64 {
    /// The 128-bit LCG state. This is the value `bit_generator.state` reports as `state`.
    state: u128,
    /// The fixed odd increment, `(initseq << 1) | 1`. Reported as `inc`.
    inc: u128,
}

impl Pcg64 {
    /// `PCG_DEFAULT_MULTIPLIER_128`: high `2549297995355413924`, low
    /// `4865540595714422341` (`pcg64.h`).
    ///
    /// **The constant's `high` half goes in the high position.** `PCG_128BIT_CONSTANT(high,
    /// low)` is `(high << 64) + low`, and swapping the two halves still yields a
    /// well-formed 128-bit LCG that produces plausible numbers — a stream that is wrong
    /// from the first draw and impossible to spot by eye.
    const MULTIPLIER: u128 = ((2549297995355413924u128) << 64) | 4865540595714422341u128;

    /// `default_rng(seed)`: hash the seed into four words, then seed PCG64 from them.
    ///
    /// Takes `u64` rather than `u32` because the entropy words are `u32` *after* the
    /// split (`_int_to_uint32_array`, lowest bits first), and a seed above `2^32` is two
    /// words and a different pool. `Ponytail (scope)`: the layout seed is always a
    /// `randint(0, 2**31 - 1)` result (`forceatlas.py:122`), so the second word is only
    /// reachable by a caller outside this crate.
    pub(crate) fn new(seed: u64) -> Self {
        let words = SeedSequence::pool(seed).generate_state();
        let initstate = ((words.state_high as u128) << 64) | words.state_low as u128;
        let initseq = ((words.inc_high as u128) << 64) | words.inc_low as u128;
        Self::from_init(initstate, initseq)
    }

    /// `pcg64_set_seed(state, seed[0..2], seed[2..4])` then
    /// `pcg_setseq_128_srandom_r`: zero the state, derive the increment, step, add the
    /// seed state, step again.
    ///
    /// The two steps are why the reported `state` is not `initstate`, and why a port that
    /// assigns `state = initstate` gets a *different* stream rather than a shifted one.
    fn from_init(initstate: u128, initseq: u128) -> Self {
        let mut rng = Self {
            state: 0,
            inc: (initseq << 1) | 1,
        };
        rng.step();
        rng.state = rng.state.wrapping_add(initstate);
        rng.step();
        rng
    }

    /// `pcg_setseq_128_step_r`: `state = state * MULTIPLIER + inc`, modulo `2^128`.
    fn step(&mut self) {
        self.state = self
            .state
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.inc);
    }

    /// `pcg64_next64`: step, then `rotr64(state.high ^ state.low, state.high >> 58)`.
    ///
    /// **Step first, output second.** `pcg64_random_r` is
    /// `pcg_setseq_128_xsl_rr_64_random_r`, which steps and only then reads the
    /// permutation. `PCG64DXSM`, the other member `default_rng` can be given, reads the
    /// *pre-iterated* state and uses a 64-bit cheap multiplier; it is a different stream
    /// from the first draw on.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.step();
        let high = (self.state >> 64) as u64;
        let low = self.state as u64;
        (high ^ low).rotate_right((high >> 58) as u32)
    }

    /// `uint64_to_double(next64)`: `(next >> 11) * 2^-53`, in that order.
    ///
    /// The shift keeps 53 bits, so the value is in `[0, 1)` on a `2^-53` lattice. Unlike
    /// [`super::Mt19937::next_f64`] this is **one** word per draw, not two: a port that
    /// split the word would have the right values and half the period.
    pub(crate) fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    /// The seeded `(state, inc)`, for the test that says the *seeding* is right on its own
    /// and not only through the stream it produces.
    #[cfg(test)]
    pub(crate) fn seeded(&self) -> (u128, u128) {
        (self.state, self.inc)
    }
}
