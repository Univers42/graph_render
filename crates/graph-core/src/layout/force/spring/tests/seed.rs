//! The start positions, pinned to the reference's own numbers at both dimensions.
//!
//! **This is the one coordinate comparison the module can make.** A force layout has no
//! closed-form coordinate to compare at a settled budget — the two arms diverge inside the
//! first step and the chaos does the rest — but the *start* is a pure function of the seed,
//! so it is exact: `np.random.RandomState(seed).rand(n, D)`, row-major. networkx turns an
//! `int` seed into exactly that (`utils/misc.py:290-291`), and SciGraphs passes
//! `seed=get_layout_seed()` (`networkx_layouts.py:16-34`).
//!
//! **Bits, never a tolerance.** A tolerance would accept a stream that is merely *near* the
//! reference; the claim is that it is the same generator drawing the same doubles, and a
//! tolerance cannot say that. So the vectors are `to_bits()` of `numpy.random.RandomState`.
//!
//! **`None` is not "the reference with a different seed" — it is a different generator.**
//! The registered default stays on `Mulberry32` so every hashed snapshot and every
//! hash-gate record stands, which is why the two streams are pinned separately below and
//! [`the_seed_parameter_reaches_the_kernel`] asserts they differ.

use super::super::{Spring, SpringParams, start};
use crate::layout::coords::probe::{graph, points};
use crate::stage::Stage;
use std::collections::HashSet;

/// `get_layout_seed()` with no pipeline seed set: `derive_seed(42, "layout")`
/// (`repro/determinism.py:124-129`).
const LAYOUT_SEED: u32 = 981_798_123;

/// `np.random.RandomState(981798123).rand(2, 3)`, row-major, as bits (numpy 2.3.3):
/// node 0 takes draws 0 and 1, node 1 draws 2 and 3, node 2 draws 4 and 5.
const RAND_2_3: [u64; 6] = [
    0x3FED_2B61_1EED_9D7B,
    0x3FE2_38BF_EACB_3589,
    0x3FD7_C4D8_0207_BEAE,
    0x3FC9_03DC_489C_5238,
    0x3FED_46A6_05A3_FD6D,
    0x3FDB_C15F_B35B_7CE4,
];

/// `Mulberry32` at `spring::SEED`, the first six draws, as bits. The stream the registered
/// `layout.force.spring` has always used; pinning it is what says `seed: None` moved no
/// hashed byte.
const MULBERRY_FIRST_SIX: [u64; 6] = [
    0x3FE6_B895_1B40_0000,
    0x3FD2_5356_F340_0000,
    0x3FEE_75FC_7F40_0000,
    0x3FBA_0CED_8300_0000,
    0x3FD8_37EF_0E80_0000,
    0x3FE4_EE6C_2DE0_0000,
];

/// The draws of one `start`, in the generator's order: all of x, then all of y, then z.
fn bits<const D: usize>(n: u32, seed: Option<u32>) -> Vec<u64> {
    start::<D>(n, seed)
        .c
        .iter()
        .flat_map(|column| column.iter().map(|v| v.to_bits()))
        .collect()
}

/// `SPRING_3D`'s start is `rand(2, 3)` — three axes, six doubles, two words of MT19937 each.
#[test]
fn a_seeded_start_is_numpys_random_sample_bit_for_bit() {
    assert_eq!(bits::<3>(2, Some(LAYOUT_SEED)), RAND_2_3.to_vec());
}

/// `SPRING`'s start is the same stream at `dim = 2`: the first four draws, not a different
/// generator. This is the arm the conformance row compares at 1020 coordinates.
#[test]
fn a_seeded_start_at_two_dimensions_is_the_same_first_four_draws() {
    assert_eq!(
        bits::<2>(2, Some(LAYOUT_SEED)),
        RAND_2_3[..4].to_vec(),
        "dim = 2 must be the same draws, truncated"
    );
}

/// The control. Without it, a `start` that ignored `seed` and hard-coded the layout seed
/// would pass both vector tests above.
#[test]
fn a_neighbouring_seed_moves_every_draw() {
    assert_ne!(bits::<3>(2, Some(LAYOUT_SEED + 1)), RAND_2_3.to_vec());
}

/// `seed: None` is the crate's own stream, byte for byte what it was before the field
/// existed: the registered layout's hashed snapshots and every hash-gate record depend on
/// it, and a change here would move all of them.
#[test]
fn the_unseeded_start_is_still_the_crates_own_mulberry32_stream() {
    assert_eq!(bits::<3>(2, None), MULBERRY_FIRST_SIX.to_vec());
    assert_ne!(
        bits::<3>(2, None),
        RAND_2_3.to_vec(),
        "the two streams must not be the same generator"
    );
}

/// The field reaches the kernel through the public stage, not only through `start`: at
/// `iterations = 0` the whole layout *is* the start, rescaled, so the two must differ and
/// both must still satisfy the rescale contract of `layout.py:646`.
#[test]
fn the_seed_parameter_reaches_the_kernel() {
    let t = graph(8, &super::path_edges(8));
    let at = |seed| {
        points(
            &Spring::run(
                &t,
                &SpringParams {
                    seed,
                    iterations: 0,
                    ..SpringParams::default()
                },
            )
            .expect("runs"),
        )
    };
    let seeded = at(Some(LAYOUT_SEED));
    let unseeded = at(None);
    assert_ne!(seeded, unseeded, "the seed is not reaching the kernel");
    for got in [&seeded, &unseeded] {
        let mean_x = got.iter().map(|p| f64::from(p.0)).sum::<f64>() / 8.0;
        let mean_y = got.iter().map(|p| f64::from(p.1)).sum::<f64>() / 8.0;
        assert!(mean_x.abs() < 1e-3 && mean_y.abs() < 1e-3, "{got:?}");
        let extent = got
            .iter()
            .map(|p| f64::from(p.0).abs().max(f64::from(p.1).abs()))
            .fold(0.0, f64::max);
        assert!((extent - 5.0).abs() < 1e-3, "span {extent}, not 5.0: {got:?}");
        let unique: HashSet<(u32, u32)> =
            got.iter().map(|p| (p.0.to_bits(), p.1.to_bits())).collect();
        assert_eq!(unique.len(), got.len(), "coincident nodes: {got:?}");
    }
}
