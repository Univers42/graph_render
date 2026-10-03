//! The reference stream, pinned bit for bit.
//!
//! **Bits and never a decimal literal with a tolerance.** A tolerance would accept a
//! stream that is merely *near* the reference, and the whole claim is that this is the
//! same generator drawing the same numbers — the layout's start positions are compared
//! against SciGraphs' own bytes.

use super::*;

/// `np.random.default_rng(1767573729).random(6)`, row-major, as bits.
///
/// 1767573729 is the seed the reference's FA2 runs on: the first draw of
/// `RandomState(981798123)`, `randint(0, 2**31 - 1)` (`forceatlas.py:122`), measured and
/// pinned in `docs/measurements/sg-fa2-seed.md`. numpy 2.3.3, `ge-python-oracle`.
const LAYOUT_SEED_DRAWS: [u64; 6] = [
    0x3FE9_6B8F_A5E5_3C4A,
    0x3FE5_525D_0587_39CE,
    0x3FE3_A71A_E513_D623,
    0x3FC1_C7A8_FC64_58F4,
    0x3FEB_5006_A7EE_BDE4,
    0x3FE9_C989_DD74_267F,
];

/// `np.random.default_rng(0).random(6)`, as bits. The seed every PCG64 test suite
/// quotes, so a wrong hash constant or a swapped multiplier half shows up here and not
/// only at the layout seed.
const ZERO_SEED_DRAWS: [u64; 6] = [
    0x3FE4_61FD_79FB_3850,
    0x3FD1_442F_7E20_B674,
    0x3FA4_FA7B_529D_9BD0,
    0x3F90_EC9E_D84D_0BC0,
    0x3FEA_064F_4F05_9BCA,
    0x3FED_354B_2F34_C803,
];

/// `np.random.default_rng(1767573729).bit_generator.state`: the 128-bit LCG state and
/// increment *after* `pcg64_set_seed`, exactly as Python reports them.
///
/// Reported separately from [`LAYOUT_SEED_DRAWS`] because they answer different
/// questions: this says the *seeding* is right, the draws say the *step and the output
/// permutation* are. A port that seeds correctly and steps wrongly passes neither.
const LAYOUT_SEED_STATE: u128 = 60223290111031304053904000740221457340;
const LAYOUT_SEED_INC: u128 = 232180640892196755669630849829386553819;

/// `np.random.default_rng(1767573729).bit_generator.random_raw(4)`: the four raw `u64`
/// draws, which are the `f64` bits shifted left by 11.
///
/// Pinned separately from [`LAYOUT_SEED_DRAWS`] because they are the *permutation*, not
/// the division: a port whose `next_f64` happened to divide the wrong word would match the
/// doubles and fail here. Two consecutive words also pin the step order — an output
/// function that read the pre-iterated state, as the DXSM member of this family does,
/// would produce the right first word and a different second one.
const LAYOUT_SEED_RAW: [u64; 4] = [
    0xCB5C_7D2F_29E2_5674,
    0xAA92_E82C_39CE_7606,
    0x9D38_D728_9EB1_1871,
    0x238F_51F8_C8B1_EC2D,
];

/// `count` `next_f64` from a fresh generator at `seed`, as bits.
fn draws(seed: u64, count: usize) -> Vec<u64> {
    let mut pcg = Pcg64::new(seed);
    (0..count).map(|_| pcg.next_f64().to_bits()).collect()
}

/// `count` `next_u64` from a fresh generator at `seed`.
fn words(seed: u64, count: usize) -> Vec<u64> {
    let mut pcg = Pcg64::new(seed);
    (0..count).map(|_| pcg.next_u64()).collect()
}

#[test]
fn pcg64_next_f64_is_numpys_default_rng_at_both_seeds() {
    assert_eq!(draws(1_767_573_729, 6), LAYOUT_SEED_DRAWS.to_vec());
    assert_eq!(draws(0, 6), ZERO_SEED_DRAWS.to_vec());
}

#[test]
fn pcg64_next_u64_is_numpys_raw_output_at_the_layout_seed() {
    assert_eq!(words(1_767_573_729, 4), LAYOUT_SEED_RAW.to_vec());
}

#[test]
fn pcg64_seeding_reaches_the_state_and_inc_numpy_reports() {
    assert_eq!(
        Pcg64::new(1_767_573_729).seeded(),
        (LAYOUT_SEED_STATE, LAYOUT_SEED_INC)
    );
}

/// The negative control. Without it, every test above would also pass against a generator
/// that ignored `seed` and hard-coded the layout seed, and a generator that got the
/// *step* wrong but the *seed* right would still look seeded.
#[test]
fn pcg64_a_neighbouring_seed_moves_every_draw_and_the_seeded_state() {
    assert_ne!(draws(1_767_573_728, 6), LAYOUT_SEED_DRAWS.to_vec());
    assert_ne!(draws(1, 6), ZERO_SEED_DRAWS.to_vec());
    assert_ne!(words(1_767_573_728, 4), LAYOUT_SEED_RAW.to_vec());
    assert_ne!(Pcg64::new(1_767_573_728).seeded().0, LAYOUT_SEED_STATE);
}

/// The 53-bit double is **one** `u64` per draw, top 53 bits: a port that split the word
/// the way [`super::Mt19937::next_f64`] splits two `u32` would produce plausible numbers
/// on half the period.
#[test]
fn pcg64_next_f64_consumes_exactly_one_word() {
    let mut raw = Pcg64::new(1_767_573_729);
    let mut doubles = Pcg64::new(1_767_573_729);
    for n in 0..6 {
        let word = raw.next_u64();
        let from_word = (word >> 11) as f64 * (1.0 / 9007199254740992.0);
        assert_eq!(from_word, doubles.next_f64(), "draw {n}");
    }
}

#[test]
fn pcg64_draws_stay_in_the_unit_interval_and_never_repeat() {
    let mut pcg = Pcg64::new(1_767_573_729);
    let mut seen = std::collections::HashSet::new();
    for n in 0..10_000u32 {
        let v = pcg.next_f64();
        assert!((0.0..1.0).contains(&v), "draw {n} at {v}");
        assert!(seen.insert(v.to_bits()), "draw {n} repeated a value");
    }
}

#[test]
fn pcg64_twice_over_is_bit_identical() {
    assert_eq!(words(1_767_573_729, 5_000), words(1_767_573_729, 5_000));
}
