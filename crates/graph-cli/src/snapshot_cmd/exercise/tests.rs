//! `Stream` is the seeded input the exercise's floats and shapes come from; `roundtrip`
//! only gets its promised coverage (subnormals, both zeros, the finite ends, both signs)
//! if the generator really produces them. Each `Stream` method is checked against an
//! independent reimplementation of the same arithmetic — copied here, never called from
//! `exercise.rs` — so a bit-op slip (`^` for `|`, `>>` for `<<`, `+` for `*`, ...) in the
//! real one shows up as a mismatch instead of hiding behind identical output bytes.

use super::*;
use std::collections::BTreeSet;

/// splitmix64's own arithmetic, kept independent of [`Stream::next`].
fn reference_next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// [`Stream::float`]'s own branching, kept independent of it.
fn reference_float(state: &mut u64) -> f32 {
    let bits = reference_next(state);
    if bits & 1 == 0 {
        return FLOATS[(bits >> 1) as usize % FLOATS.len()];
    }
    let value = f32::from_bits((bits >> 32) as u32);
    if value.is_finite() {
        value
    } else {
        f32::from_bits(value.to_bits() & 0xff7f_ffff)
    }
}

#[test]
fn next_matches_an_independent_splitmix64_bit_for_bit() {
    for seed in [0u64, 1, 2, 42, 1_000_003, u64::MAX, 0x9E37_79B9_7F4A_7C15] {
        let mut want = seed;
        let mut s = Stream(seed);
        for draw in 0..8 {
            assert_eq!(
                s.next(),
                reference_next(&mut want),
                "seed {seed} draw {draw}"
            );
        }
    }
}

#[test]
fn float_matches_an_independent_reimplementation_bit_for_bit() {
    for seed in 0..500u64 {
        let mut want = seed;
        let mut s = Stream(seed);
        for draw in 0..4 {
            assert_eq!(
                s.float().to_bits(),
                reference_float(&mut want).to_bits(),
                "seed {seed} draw {draw}"
            );
        }
    }
}

#[test]
fn floats_include_both_signed_zeros_with_distinct_bits() {
    assert_eq!(FLOATS[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(FLOATS[1].to_bits(), (-0.0f32).to_bits());
    assert_ne!(FLOATS[0].to_bits(), FLOATS[1].to_bits());
}

#[test]
fn floats_include_one_third_not_a_look_alike() {
    // 1.0 / 3.0, not 1.0 % 3.0 (== 1.0) nor 1.0 * 3.0 (== 3.0).
    assert_eq!(FLOATS[8], 1.0f32 / 3.0f32);
}

#[test]
fn floats_include_a_negative_near_the_integer_edge() {
    assert_eq!(FLOATS[11], -8_388_607.5);
}

#[test]
fn draws_cover_every_pinned_float_including_subnormals_and_extremes() {
    let mut seen = BTreeSet::new();
    for seed in 0..3_000u64 {
        let mut s = Stream(seed);
        for _ in 0..8 {
            seen.insert(s.float().to_bits());
        }
    }
    for want in FLOATS {
        assert!(seen.contains(&want.to_bits()), "{want} never drawn");
    }
}

#[test]
fn seed_maps_to_the_documented_node_and_edge_counts() {
    // 63 = lcm(9, 7): enough seeds to see every remainder of both moduli.
    for seed in 0..63u32 {
        let want_n = 1 + seed % 9;
        let want_m = seed % 7;
        let s = snapshot(seed).expect("valid");
        let h = s.header();
        assert_eq!(h.node_count, want_n, "seed {seed}");
        assert_eq!(h.edge_count, want_m, "seed {seed}");
    }
}

#[test]
fn odd_seeds_blank_the_first_node_id_even_seeds_do_not() {
    for seed in [0u32, 1, 2, 3, 100, 101] {
        let parts = snapshot(seed).expect("valid").into_parts();
        let first = parts.node_ids.get(0).expect("has a node");
        if seed % 2 == 1 {
            assert_eq!(first, "", "seed {seed}");
        } else {
            assert_ne!(first, "", "seed {seed}");
        }
    }
}

#[test]
fn edge_path_offsets_accumulate_instead_of_collapsing_to_zero() {
    let mut s = Stream(7);
    let geo = edges(&mut s, 1, 20);
    let EdgeGeometry::Polyline(paths) = geo else {
        panic!("kind 1 is a polyline");
    };
    assert_eq!(paths.offsets[0], 0);
    let last = *paths.offsets.last().expect("has an end offset");
    assert!(last > 0, "20 edges never grew past 0: {:?}", paths.offsets);
    assert_eq!(paths.pts.len(), 2 * last as usize);
}
