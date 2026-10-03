//! Explicit, threaded randomness for layout (`prompt.md` §6, devil C8). Three kinds:
//!
//! - **Sequential** — initial state *outside* the per-tick kernels (e.g. a layout's start
//!   positions). [`Mulberry32`] already exists in `synthetic.rs`; this module re-exports
//!   it rather than adding a second sequential generator, per C8's instruction.
//! - **Counter-based** — randomness *inside* a kernel (Barnes-Hut/link/collide jiggle for
//!   exactly-coincident points). d3 draws jiggle from one shared LCG consumed in quadtree
//!   visit order (`d3-force/src/jiggle.js`, `lcg.js`): a sequential generator threaded
//!   through a gather would make the result depend on visit order, and later on thread
//!   order (Phase 11) — exactly what D10 forbids. [`jiggle`] is a pure function of
//!   `(seed, tick, pass, i, j)` instead: order-independent and thread-safe for free.
//! - **The reference's own generator** — [`Mt19937`], numpy's legacy `RandomState`, and
//!   [`Pcg64`], numpy's modern `default_rng` (PCG64), for the layouts whose conformance
//!   rows compare bytes against SciGraphs. They exist beside [`Mulberry32`] rather than
//!   replacing it: only a SciGraphs arm needs these exact numbers, and a motor-side default
//!   stays on the crate's own stream.

mod pcg64;

pub(crate) use pcg64::Pcg64;
pub(crate) use crate::synthetic::Mulberry32;

/// MurmurHash3's `fmix64` finalizer (Austin Appleby, public domain): full avalanche of a
/// 64-bit word using only xor/shift/`wrapping_mul` — never a transcendental, so it is
/// exact and bit-identical on every target (D1, D2 do not even apply: there is no float
/// here until the final division).
const fn fmix64(mut k: u64) -> u64 {
    k ^= k >> 33;
    k = k.wrapping_mul(0xff51_afd7_ed55_8ccd);
    k ^= k >> 33;
    k = k.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    k ^= k >> 33;
    k
}

/// Folds `(seed, tick, pass, lo, hi)` into one 64-bit word, three avalanche rounds deep so
/// a one-bit change in any input differs in about half the output bits. `(lo, hi)` travel
/// as one pair, not two more parameters (house cap: `refactor-rust.md`'s max 4).
const fn fold(seed: u32, tick: u32, pass: u32, pair: (u32, u32)) -> u64 {
    let (lo, hi) = pair;
    let a = fmix64((seed as u64) ^ ((tick as u64) << 32));
    let b = fmix64(a ^ ((pass as u64) << 32) ^ lo as u64);
    fmix64(b ^ ((hi as u64) << 32) ^ a)
}

/// A stateless, counter-based nudge for two exactly-coincident points, the same shape as
/// d3's `jiggle(random) = (random() - 0.5) * 1e-6` (`jiggle.js:1-3`) but keyed on
/// `(seed, tick, pass, min(i, j), max(i, j))` instead of shared sequential state: node `i`
/// and node `j` computing the *same* pair see the same nudge no matter which one runs
/// first, or on which thread (C8) — the property a sequential LCG cannot offer a gather.
///
/// `pass` distinguishes kernels sharing one tick (link vs many-body vs collide) so they do
/// not accidentally draw the same stream.
///
/// Ponytail: exactly-coincident points are the only failing input this exists for; the
/// nudge is a deterministic direction, not a physically meaningful one — two nodes at the
/// same point separate the same way every run, never a different way. Escape hatch: the
/// caller supplies `tick`/`pass` so a still-coincident pair keeps nudging on later ticks
/// instead of sticking. `i, j` travel as one pair, not two more parameters (house cap:
/// `refactor-rust.md`'s max 4) — order does not matter, [`fold`] sorts them.
pub(crate) fn jiggle(seed: u32, tick: u32, pass: u32, ij: (u32, u32)) -> f64 {
    let (i, j) = ij;
    let lo_hi = if i <= j { (i, j) } else { (j, i) };
    let h = fold(seed, tick, pass, lo_hi);
    let u = (h >> 11) as f64 * (1.0 / (1u64 << 53) as f64);
    (u - 0.5) * 1e-6
}

/// The generator the *reference* draws from, ported so a coordinate can be SciGraphs'
/// rather than merely like it.
///
/// **numpy's legacy `RandomState`, seeded the legacy way.** Two separate facts, and confusing
/// them is the easy way to get a stream that is nearly right:
///
/// - **The double.** `random_sample` is the 53-bit `genrand_res53` construction below — 27
///   bits from one tempered word and 26 from the next. The modern `Generator.random()` uses
///   that same two-word double, so the double is not what distinguishes the two APIs.
/// - **The seed.** `RandomState(seed)` runs `init_genrand(seed)` (`_mt19937.pyx`): `mt[0] =
///   seed`, then `mt[i] = 1812433253 * (mt[i-1] ^ (mt[i-1] >> 30)) + i`. The modern
///   `Generator(MT19937(seed))` does **not** — its state word 0 is `0x8000_0000`, not the
///   seed, so `Generator(MT19937(s)).random()` and `RandomState(s).rand()` disagree from the
///   first value even at the same integer `s` (measured, numpy 2.3.3, `s = 981798123`:
///   `0x1.d8b00910d3e10p-3` against `0x1.d2b611eed9d7bp-1`). Seeding is therefore part of the
///   port, and the pinned vectors below are what say which stream this is.
///
/// The state is **integer-only** and every step is xor/shift/`wrapping_*`, so the stream is
/// bit-identical on every target (D1, D2, D10). The one float is the final division in
/// [`Mt19937::next_f64`], in the order the reference writes it.
pub(crate) struct Mt19937 {
    /// The tempered-word block; `index` reads it, `twist` rewrites it in place.
    mt: [u32; Self::N],
    index: usize,
}

impl Mt19937 {
    /// The state block size, `N` in the reference's `_mt19937.pyx`.
    const N: usize = 624;
    /// The twist's recurrence offset, `M`: where the "two blocks ago" word comes from.
    const M: usize = 397;
    /// The twist's odd-word xor, `MATRIX_A`.
    const MATRIX_A: u32 = 0x9908_b0df;

    /// `init_genrand(seed)`: `mt[0] = seed`, then the reference's own recurrence — every
    /// later word depends on the one before it, so the block must be built in order and
    /// cannot be filled in parallel.
    pub(crate) fn new(seed: u32) -> Self {
        let mut mt = [0u32; Self::N];
        mt[0] = seed;
        for i in 1..Self::N {
            let prev = mt[i - 1];
            mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        Self { mt, index: Self::N }
    }

    /// The twist: one pass that folds the block's lag-`M` words back into it. Runs only
    /// when the block is spent, which is what makes the stream sequential.
    fn twist(&mut self) {
        for i in 0..Self::N {
            let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % Self::N] & 0x7fff_ffff);
            let mut word = self.mt[(i + Self::M) % Self::N] ^ (y >> 1);
            if y & 1 == 1 {
                word ^= Self::MATRIX_A;
            }
            self.mt[i] = word;
        }
        self.index = 0;
    }

    /// `genrand_uint32`: the next tempered word, twisting first if the block is spent.
    pub(crate) fn next_u32(&mut self) -> u32 {
        if self.index >= Self::N {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `random_sample`, the `f64` in `[0, 1)`: `(a * 2^26 + b) / 2^53` with `a` the top 27
    /// bits of one word and `b` the top 26 of the next, in that order.
    ///
    /// **Two words per draw, not one.** 27 + 26 is the 53 bits a double keeps, and the two
    /// halves come from two words because that is where they are; a generator that drew one
    /// word per value would be a different stream even with the same seeding.
    pub(crate) fn next_f64(&mut self) -> f64 {
        let a = f64::from(self.next_u32() >> 5);
        let b = f64::from(self.next_u32() >> 6);
        (a * 67108864.0 + b) / 9007199254740992.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `np.random.RandomState(981798123).rand(2, 3)`, row-major, as bits. numpy 2.3.3.
    ///
    /// Bits and never a decimal literal with a tolerance: a tolerance would accept a stream
    /// that is merely *near* the reference, and the whole claim is that it is the same
    /// generator drawing the same numbers.
    const LAYOUT_SEED_DRAWS: [u64; 6] = [
        0x3FED_2B61_1EED_9D7B,
        0x3FE2_38BF_EACB_3589,
        0x3FD7_C4D8_0207_BEAE,
        0x3FC9_03DC_489C_5238,
        0x3FED_46A6_05A3_FD6D,
        0x3FDB_C15F_B35B_7CE4,
    ];

    /// `np.random.RandomState(0).rand(3)`, as bits: the seed every MT19937 test suite
    /// quotes, so a wrong seeding constant shows up here and not only at the layout seed.
    const ZERO_SEED_DRAWS: [u64; 3] = [
        0x3FE1_8FE1_565F_12A8,
        0x3FE6_E2D4_CF60_8733,
        0x3FE3_49D6_6B6E_894B,
    ];

    /// The first four `next_u32` of `seed`, as numpy's `randint` reports them.
    const ZERO_SEED_U32: [u32; 4] = [2_357_136_044, 2_546_248_239, 3_071_714_933, 3_626_093_760];
    const LAYOUT_SEED_U32: [u32; 4] = [3_915_057_377, 3_144_113_877, 2_445_672_285, 2_999_804_494];

    /// `count` `next_f64` from a fresh generator at `seed`, as bits.
    fn draws(seed: u32, count: usize) -> Vec<u64> {
        let mut mt = Mt19937::new(seed);
        (0..count).map(|_| mt.next_f64().to_bits()).collect()
    }

    /// `count` `next_u32` from a fresh generator at `seed`.
    fn words(seed: u32, count: usize) -> Vec<u32> {
        let mut mt = Mt19937::new(seed);
        (0..count).map(|_| mt.next_u32()).collect()
    }

    #[test]
    fn mt19937_next_f64_is_numpys_random_sample_at_both_seeds() {
        assert_eq!(draws(981_798_123, 6), LAYOUT_SEED_DRAWS.to_vec());
        assert_eq!(draws(0, 3), ZERO_SEED_DRAWS.to_vec());
    }

    #[test]
    fn mt19937_next_u32_is_numpys_genrand_uint32_at_both_seeds() {
        assert_eq!(words(0, 4), ZERO_SEED_U32.to_vec());
        assert_eq!(words(981_798_123, 4), LAYOUT_SEED_U32.to_vec());
    }

    /// The 53-bit double is **two** `u32` per draw: `a >> 5` gives 27 bits and `b >> 6`
    /// gives 26, so a generator that drew one word per value would still produce plausible
    /// numbers. Paired words consumed in order must reproduce `next_f64` exactly.
    #[test]
    fn mt19937_next_f64_consumes_exactly_two_words_in_order() {
        let mut words = Mt19937::new(981_798_123);
        let mut doubles = Mt19937::new(981_798_123);
        for n in 0..6 {
            let a = f64::from(words.next_u32() >> 5);
            let b = f64::from(words.next_u32() >> 6);
            let from_words = (a * 67108864.0 + b) / 9007199254740992.0;
            assert_eq!(from_words, doubles.next_f64(), "draw {n}");
        }
    }

    /// The control: a stream seeded one higher must not reproduce the reference's numbers.
    /// Without this the vector test would also pass against a generator that ignores
    /// `seed` and hard-codes the layout seed.
    #[test]
    fn mt19937_a_neighbouring_seed_moves_every_draw() {
        assert_ne!(draws(981_798_122, 6), LAYOUT_SEED_DRAWS.to_vec());
        assert_ne!(words(1, 4), ZERO_SEED_U32.to_vec());
    }

    #[test]
    fn mt19937_draws_stay_in_the_unit_interval_and_never_repeat() {
        let mut mt = Mt19937::new(981_798_123);
        let mut seen = std::collections::HashSet::new();
        for n in 0..10_000 {
            let v = mt.next_f64();
            assert!((0.0..1.0).contains(&v), "draw {n} at {v}");
            assert!(seen.insert(v.to_bits()), "draw {n} repeated a value");
        }
    }

    #[test]
    fn mt19937_twice_over_is_bit_identical() {
        assert_eq!(draws(981_798_123, 5_000), draws(981_798_123, 5_000));
    }

    #[test]
    fn jiggle_is_order_independent_in_i_and_j() {
        for (i, j) in [(0u32, 1u32), (5, 2), (100, 99), (0, 0)] {
            assert_eq!(jiggle(7, 3, 0, (i, j)), jiggle(7, 3, 0, (j, i)));
        }
    }

    #[test]
    fn jiggle_is_a_pure_function_of_its_five_inputs() {
        let a = jiggle(1, 2, 3, (4, 5));
        let b = jiggle(1, 2, 3, (4, 5));
        assert_eq!(a.to_bits(), b.to_bits());
        assert_ne!(a.to_bits(), jiggle(1, 2, 3, (4, 6)).to_bits(), "j moved");
        assert_ne!(a.to_bits(), jiggle(1, 2, 4, (4, 5)).to_bits(), "pass moved");
        assert_ne!(a.to_bits(), jiggle(1, 3, 3, (4, 5)).to_bits(), "tick moved");
        assert_ne!(a.to_bits(), jiggle(2, 2, 3, (4, 5)).to_bits(), "seed moved");
    }

    #[test]
    fn jiggle_stays_in_d3s_magnitude_band_and_is_finite() {
        for n in 0..2_000u32 {
            let v = jiggle(0x9E37_79B9, n / 37, n % 5, (n, n.wrapping_mul(31)));
            assert!(v.is_finite(), "{n}: {v}");
            assert!((-5e-7..5e-7).contains(&v), "{n}: {v} out of band");
        }
    }

    #[test]
    fn run_twice_is_bit_identical_determinism() {
        let run = || {
            (0..500)
                .map(|n| jiggle(9, n, 1, (n, n + 1)).to_bits())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
