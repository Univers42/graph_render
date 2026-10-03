//! `rng.rs`'s own test, split out for the house 300-line cap — the same split
//! `link.rs`, `charge.rs` and `tests.rs` already use.

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

/// [`jiggle`]'s value for the one 53-bit word whose raw value is `+0.0`. Every call
/// site's zero-jiggle guard assumes this cannot happen, so the mapping has to be here,
/// in the one function, rather than at each of the guards.
///
/// It is a test on `jiggle_of` and not on `jiggle` because the word is unreachable
/// without a pre-image of `fmix64`: a scan of 1 638 400 draws found none, so a test on
/// `jiggle` itself could only assert the absence of a defect and would pass against the
/// broken mapping too.
#[test]
fn the_midpoint_word_never_becomes_a_zero_nudge() {
    assert_ne!(jiggle_of(1 << 52), 0.0);
    // and the value is a nudge of d3's magnitude, not merely "not zero"
    assert!((-5e-7..5e-7).contains(&jiggle_of(1 << 52)));
}

/// The control for the mapping: exactly one word moves. Every other word — including
/// both neighbours of the midpoint, `u == 0.5 ± 2^-53` — keeps the value the raw
/// formula gives, to the bit. If the guard were a clamp or a wider branch, one of these
/// would move too, which is the only way a change this small could shift a layout.
#[test]
fn the_midpoint_word_is_the_only_one_the_mapping_moves() {
    let raw = |w: u64| (w as f64 * (1.0 / (1u64 << 53) as f64) - 0.5) * 1e-6;
    for w in [0u64, 1, (1 << 52) - 1, (1 << 52) + 1, (1 << 53) - 1] {
        assert_eq!(jiggle_of(w).to_bits(), raw(w).to_bits(), "word {w} moved");
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
