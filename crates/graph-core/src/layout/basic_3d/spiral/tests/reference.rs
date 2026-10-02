//! The numpy-pinned half of [`spiral`](super::super)'s test suite: the oracle's own
//! IEEE-754 words, the helpers that compare against them, and the three tests that do.

use crate::layout::basic_3d::spiral;
use crate::layout::basic_3d::tests::{bare, space};

/// The IEEE-754 bits of an `f64`, big-endian word order: what the oracle printed.
fn bits(value: f64) -> u64 {
    value.to_bits()
}

/// The distance between two `f64` in units in the last place.
///
/// **Over the ordinal, not over the encoding**: the raw bits are not monotone across the
/// sign bit, and `+0.0` and `-0.0` must be one value rather than 1.8e308 ulp apart. So the
/// encoding is folded into a monotone `i64` first, the same convention
/// `docs/measurements/scigraphs-conformance.md:108` states for the matrix's own column.
fn ulps(got: f64, want: f64) -> i64 {
    fn ordinal(value: f64) -> i64 {
        let word = value.to_bits() as i64;
        if word < 0 { i64::MIN - word } else { word }
    }
    (ordinal(got) - ordinal(want)).abs()
}

/// `_spiral_layout_3d(1, 5.0)` — the `num_nodes == 1` branch of `basic.py:52-55`.
///
/// A single node takes **`0.5*length[-1]`**, the midpoint of the whole arc, so it sits at
/// `r = 3.75`, `z = 0`… except the z here is `+0.775`, not 0, because `t` is the
/// *parameter* at half the arc, and half the parameter is not half the height: the climb is
/// linear in `t`, not in arc length. This is the single most informative number in the
/// whole port, because it is the only one that cannot be reproduced by any shortcut.
const N1_T: [u64; 1] = [0x3fe2_7afb_7a4c_0736];
const N1_X: [u64; 1] = [0x4001_bad2_7fc7_6001];
const N1_Y: [u64; 1] = [0x400a_18ee_0bc4_662d];
const N1_Z: [u64; 1] = [0x3fe8_cdd2_c6f8_481c];

/// `_spiral_layout_3d(2, 5.0)` — the smallest case where `wanted` is a `linspace`.
///
/// `t[0] = 0` exactly (`wanted[0] = start`, and `interp` at `xp[0]` returns `fp[0]`) and
/// `t[1] = 1` exactly (the `linspace` final-point fix-up, so `x == xp[-1]`, and the slope
/// branch lands back on `fp[-1] = 1.0`). So the two ends are the reference's own endpoints
/// and need no interpolation to be believed.
///
/// `y[1] = -2.4492935982947065e-15` is `5.0 * sin(4*pi)`, i.e. **`libm` and numpy agree
/// here**: `sin` of the nearest `f64` to `4*pi` is `-4.898587196589413e-16` on both, and
/// the radius multiplies it. It is small, it is not zero, and it is reproducible.
const N2_T: [u64; 2] = [0x0000_0000_0000_0000, 0x3ff0_0000_0000_0000];
const N2_X: [u64; 2] = [0x4004_0000_0000_0000, 0x4014_0000_0000_0000];
const N2_Y: [u64; 2] = [0x0000_0000_0000_0000, 0xbce6_0faf_bfd9_7309];
const N2_Z: [u64; 2] = [0xc014_0000_0000_0000, 0x4014_0000_0000_0000];

/// `_spiral_layout_3d(7, 5.0)` — odd, so `turns` is `max(2, round(sqrt(7/(0.75*pi))))`
/// with `sqrt(2.9708...) = 1.7235...` rounding down to `1` and then floored up to `2`.
///
/// Seven nodes over two turns is 0.5 of a turn between nodes 0 and 1 at the foot and
/// nearly 0.1 of a turn at the top, which is the whole point of the construction: the
/// spacing is even in **arc length**, not in angle, so the top of the spiral — where the
/// circumference is twice as long — carries the same number of nodes per unit of curve.
const N7_T: [u64; 7] = [
    0x0000_0000_0000_0000,
    0x3fcc_4eac_e27c_7675,
    0x3fda_3e27_b155_cae1,
    0x3fe2_7afb_7a4c_0736,
    0x3fe7_57ab_5b9c_c02a,
    0x3feb_d2c5_eed0_211f,
    0x3ff0_0000_0000_0000,
];
const N7_X: [u64; 7] = [
    0x4004_0000_0000_0000,
    0xc006_d5ea_6a6e_19ca,
    0x3ff8_0aed_1c5f_a525,
    0x4001_bad2_7fc7_6001,
    0xc010_b89d_9562_65de,
    0xbfd4_bc35_eb1b_cf41,
    0x4014_0000_0000_0000,
];
const N7_Y: [u64; 7] = [
    0x0000_0000_0000_0000,
    0x3ff1_529b_bcb1_a626,
    0xc009_82a4_90bc_2666,
    0x400a_18ee_0bc4_662d,
    0x3ff1_aa40_1633_3bd4,
    0xc012_a65a_86ca_3fec,
    0xbce6_0faf_bfd9_7309,
];
const N7_Z: [u64; 7] = [
    0xc014_0000_0000_0000,
    0xc006_4ed3_f272_35f8,
    0xbfec_c939_8953_099b,
    0x3fe8_cdd2_c6f8_481c,
    0x4002_5b2c_6507_e069,
    0x400d_8eee_d508_52ce,
    0x4014_0000_0000_0000,
];

/// The `f64` words of `t` and `z` are the reference's, bit for bit, at all three sizes.
///
/// **These two columns are the whole port.** `t` is where the entire arc-length inversion
/// lives — `linspace`'s endpoint fix-up, the sequential 65 535-addition `cumsum`, and
/// `interp`'s slope formula, all three of them — and `z = s*(2t - 1)` is a second,
/// independent read on the same `t` that no trigonometric approximation can flatter. A port
/// that got any of the three wrong cannot produce these two columns, at any size.
#[test]
fn the_arc_length_inversion_and_z_are_the_reference_bit_for_bit() {
    for (n, want_t, want_z) in [
        (1u32, &N1_T[..], &N1_Z[..]),
        (2, &N2_T[..], &N2_Z[..]),
        (7, &N7_T[..], &N7_Z[..]),
    ] {
        let t = super::super::parameters(n);
        let (_, _, z) = super::super::columns(n);
        assert_eq!(t.len(), n as usize, "n={n}: t has one entry per node");
        for i in 0..n as usize {
            assert_eq!(bits(t[i]), want_t[i], "n={n} node {i}: t");
            assert_eq!(bits(z[i]), want_z[i], "n={n} node {i}: z");
        }
    }
}

/// The trig columns agree with the reference to **one `f64` ulp**, which is what two
/// different `sin`/`cos` implementations can promise.
///
/// `libm` is D1's requirement for every transcendental in this tree and numpy's array loops
/// are the reference's own, and they are not the same code: the `f64` words of `x` and `y`
/// are therefore **not structurally reachable** and the row's tier is `tolerance`, not
/// `bitwise`. One ulp is the whole of the disagreement — see
/// [`the_trig_columns_agree_with_the_reference_to_the_last_bit_the_f32_keeps`] for the
/// assertion that does have to hold.
#[test]
fn the_trig_columns_are_the_reference_within_one_f64_ulp() {
    for (n, want_x, want_y) in [
        (1u32, &N1_X[..], &N1_Y[..]),
        (2, &N2_X[..], &N2_Y[..]),
        (7, &N7_X[..], &N7_Y[..]),
    ] {
        let (x, y, _) = super::super::columns(n);
        for i in 0..n as usize {
            let dx = ulps(x[i], f64::from_bits(want_x[i]));
            let dy = ulps(y[i], f64::from_bits(want_y[i]));
            assert!(dx <= 1, "n={n} node {i}: x is {dx} ulp from the reference");
            assert!(dy <= 1, "n={n} node {i}: y is {dy} ulp from the reference");
        }
    }
}

/// The trig columns agree with the reference to the last bit the `f32` keeps — which is the
/// one that is reachable, because the `f32` is what the motor actually ships
/// (`basic_3d.rs:50-61` narrows once).
///
/// This is the assertion the row's `f32 k/N` cell stands on: every `x` and `y` SciGraphs
/// computes must survive the narrowing to the same 32 bits here. The `f64` ulp difference
/// above is the whole of what `tolerance` costs and none of it reaches this far. If this
/// ever fails, the layout has drifted by more than an `f32` ulp and the row's claim is gone.
#[test]
fn the_trig_columns_agree_with_the_reference_to_the_last_bit_the_f32_keeps() {
    for (n, want_x, want_y) in [
        (1u32, &N1_X[..], &N1_Y[..]),
        (2, &N2_X[..], &N2_Y[..]),
        (7, &N7_X[..], &N7_Y[..]),
    ] {
        let (xf, yf, _) = space(&spiral(&bare(n)).expect("runs"));
        for i in 0..n as usize {
            assert_eq!(
                xf[i].to_bits(),
                (f64::from_bits(want_x[i]) as f32).to_bits(),
                "n={n} node {i}: x"
            );
            assert_eq!(
                yf[i].to_bits(),
                (f64::from_bits(want_y[i]) as f32).to_bits(),
                "n={n} node {i}: y"
            );
        }
    }
}
