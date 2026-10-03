//! The structural half of [`spiral`](super::super)'s test suite: every property that is a
//! claim about the construction itself rather than about the oracle's transcribed words.
//!
//! Nothing here is pinned to a bit pattern except the grid's own `linspace` step, which is
//! a closed form and so is pinned the same way the reference's is. `bits` is duplicated from
//! [`reference`](super::reference) rather than hoisted into the parent, which keeps both
//! children readable on their own and the parent to nothing but two `mod` lines.

use crate::layout::basic_3d::spiral;
use crate::layout::basic_3d::tests::{bare, space};

/// The IEEE-754 bits of an `f64`, big-endian word order: what the oracle printed.
fn bits(value: f64) -> u64 {
    value.to_bits()
}

/// `z` is the `t` column read through a second, transcendental-free formula, so the two
/// must agree to the last bit — and both are the reference's.
#[test]
fn z_is_the_parameter_column_through_a_second_formula() {
    for n in [1u32, 2, 7, 33] {
        let t = super::super::parameters(n, super::super::SCALE);
        let (_, _, z) = super::super::columns(n);
        for i in 0..n as usize {
            assert_eq!(
                bits(z[i]),
                bits(5.0 * (2.0 * t[i] - 1.0)),
                "n={n} node {i}: z from t"
            );
        }
    }
}

/// The narrowing happens exactly once, in `in_space`, and nowhere else.
#[test]
fn the_narrowed_columns_are_the_f64_columns_rounded_once() {
    let (xf, yf, zf) = space(&spiral(&bare(7)).expect("runs"));
    let (x, y, z) = super::super::columns(7);
    for i in 0..7 {
        assert_eq!(xf[i].to_bits(), (x[i] as f32).to_bits(), "node {i}: x");
        assert_eq!(yf[i].to_bits(), (y[i] as f32).to_bits(), "node {i}: y");
        assert_eq!(zf[i].to_bits(), (z[i] as f32).to_bits(), "node {i}: z");
    }
}

/// `t` climbs monotonically from the foot to the head and never leaves `[0, 1]`.
///
/// Monotone because `wanted` is a `linspace` and `interp` on a monotone `xp` is monotone;
/// inside the unit interval because `grid` is a `linspace` of `[0, 1]`. Together they say
/// the interpolation never overshoots — the failure mode of the naive
/// `t = wanted/length[-1]` shortcut, which this asserts against by never seeing a value
/// outside the range the reference's own `fp` spans.
#[test]
fn t_is_monotone_and_inside_the_unit_interval() {
    for n in [1u32, 2, 3, 7, 15, 77, 256] {
        let t = super::super::parameters(n, super::super::SCALE);
        assert!(
            t.iter().all(|v| (0.0..=1.0).contains(v)),
            "n={n}: t left [0,1]"
        );
        assert!(
            t.windows(2).all(|w| w[0] <= w[1]),
            "n={n}: t is not monotone"
        );
    }
}

/// The two ends are the reference's own endpoints: `t = 0` at the foot, `t = 1` at the head.
///
/// Both are exactly representable, and neither is an approximation this port had to rescue:
/// `grid[0] = 0 * step = 0.0`, and `grid[65535] = 65535 * step = 1.0` **exactly** — see
/// `the_grid_last_point_is_the_product_not_a_fix_up`, which is where that product is pinned.
/// An earlier version of this test claimed the head would land at `0.9999999999999986`
/// without the reference's endpoint overwrite; it would not, and the number was never real.
#[test]
fn the_two_ends_are_the_reference_endpoints_and_not_its_approximation() {
    for n in [2u32, 3, 7, 77] {
        let t = super::super::parameters(n, super::super::SCALE);
        assert_eq!(t[0], 0.0, "n={n}: the foot is grid[0] exactly");
        assert_eq!(
            t[n as usize - 1],
            1.0,
            "n={n}: the head is grid[-1] exactly"
        );
    }
}

/// The head node is at radius `scale` and height `+scale`, the foot at `scale/2` and
/// `-scale` — the endpoints the function's own docstring names (`basic.py:37-38`).
#[test]
fn the_cone_spans_the_radii_and_the_heights_its_docstring_names() {
    let (x, y, z) = super::super::columns(7);
    assert_eq!(
        (x[0], y[0], z[0]),
        (2.5, 0.0, -5.0),
        "the foot: r = scale/2, z = -scale"
    );
    assert!(
        (libm::sqrt(x[6] * x[6] + y[6] * y[6]) - 5.0).abs() < 1e-12,
        "the head is at radius scale"
    );
    assert!(
        (z[6] - 5.0).abs() < 1e-12,
        "the head is at z = +scale, got {}",
        z[6]
    );
}

/// `turns = max(2, round(sqrt(n / (0.75*pi))))` (`basic.py:41`), with `round` half-to-even.
///
/// **The floor lifts the value at exactly `n = 1..5`.** The raw rounded value is 1 there and
/// already 2 for `n = 6..14`, so from 6 upward the floor changes nothing and `n = 7` — a size
/// a reader reaches for first — would be 2 with or without it. A port without the `max(2, ...)`
/// draws a single-turn spiral at `n = 1..5` and every pinned `t` there moves. `n = 15` is the
/// first count that rounds above 2, and `n = 77` is the largest fixture.
#[test]
fn the_turn_count_is_the_references_count_with_its_floor() {
    // The floor lifts the value at exactly n = 1..5; from 6 to 14 the rounding already says 2.
    for (n, want) in [
        (1u32, 2u32),
        (2, 2),
        (3, 2),
        (4, 2),
        (5, 2),
        (6, 2),
        (7, 2),
        (14, 2),
        (15, 3),
        (77, 6),
        (200, 9),
        (256, 10),
    ] {
        assert_eq!(super::super::turns(n), want, "n={n}: turns");
    }
}

/// **The floor changes nothing at `n >= 6`, and this is the row that says so.** An earlier
/// version of this module claimed the floor applied across `n <= 14`; it does not. The raw
/// rounded value `round(sqrt(n/(0.75*pi)))` is 1 for `n = 1..5` and already 2 for `n = 6..14`,
/// so from 6 upward `max(2, ...)` is a no-op and a port that dropped it would still be right
/// everywhere except the five smallest graphs.
///
/// The point of asserting it separately is that it is the claim most likely to be repeated
/// from the doc comment, and it was wrong there.
#[test]
fn the_floor_lifts_only_the_five_smallest_counts() {
    for n in 6u32..=14 {
        let raw = libm::sqrt(f64::from(n) / (0.75 * core::f64::consts::PI)).round_ties_even();
        assert_eq!(raw, 2.0, "n={n}: the raw rounded value is already 2");
        assert_eq!(
            super::super::turns(n),
            raw as u32,
            "n={n}: the floor is a no-op here"
        );
    }
    for n in 1u32..=5 {
        let raw = libm::sqrt(f64::from(n) / (0.75 * core::f64::consts::PI)).round_ties_even();
        assert_eq!(raw, 1.0, "n={n}: the floor is what lifts this one");
        assert_eq!(super::super::turns(n), 2, "n={n}: floored to 2");
    }
}

/// `interp` is a total function: outside the arc it returns the endpoint values, which are
/// numpy's `left`/`right` defaults (`fp[0]` and `fp[-1]`).
///
/// Unreachable from this layout — `wanted` never leaves `[length[0], length[-1]]` — and
/// asserted anyway because the alternative is an out-of-bounds index on a silently
/// different answer, and because a reader porting this wants to know what the ends do.
#[test]
fn interp_returns_the_endpoint_values_outside_the_arc() {
    let step = super::super::grid_step();
    let length = super::super::arc_length(step, super::super::omega(7), super::super::SCALE);
    let last = length.len() - 1;
    assert_eq!(
        super::super::interp(-1.0, &length, step),
        0.0,
        "left is fp[0] = grid[0]"
    );
    assert_eq!(
        super::super::interp(f64::MAX, &length, step),
        1.0,
        "right is fp[-1] = 1.0"
    );
    // And on the arc it is the interior of the table, not an endpoint.
    let mid = super::super::interp(length[last] / 2.0, &length, step);
    assert!(
        (0.0..1.0).contains(&mid),
        "the midpoint of the arc is interior, got {mid}"
    );
}

/// The arc-length table is built once and is **sequential**, so the value depends on the
/// order of 65 535 additions. A blocked or pairwise reduction lands a few ulp away and
/// every interpolated `t` inherits the error.
#[test]
fn the_arc_length_table_is_monotone_and_starts_at_zero() {
    let step = super::super::grid_step();
    let length = super::super::arc_length(step, super::super::omega(7), super::super::SCALE);
    assert_eq!(
        length.len(),
        1 << 16,
        "the reference's grid is 1 << 16 points"
    );
    assert_eq!(length[0], 0.0, "basic.py:50 concatenates a leading 0.0");
    assert!(
        length.windows(2).all(|w| w[0] < w[1]),
        "a strictly increasing table is what interp requires"
    );
}

/// The grid's own step is `1/(65535)` as a single `f64` division.
#[test]
fn the_grid_is_a_linspace_with_its_step_one_division() {
    let step = super::super::grid_step();
    assert_eq!(bits(step), bits(1.0 / 65535.0), "delta/div, one division");
    assert_eq!(super::super::grid_at(0, step), 0.0);
    assert_eq!(
        super::super::grid_at(1, step),
        step,
        "and the step IS grid[1] - grid[0]"
    );
}

/// `65535 * step` is **exactly `1.0`**, so the reference's `linspace` endpoint overwrite and a
/// plain `j * step` agree bit for bit and [`grid_at`](super::super::grid_at) carries no
/// last-element special case.
///
/// This is the test that replaces the dead branch an earlier version of the module had. That
/// branch returned a hardcoded `1.0` for `j == GRID - 1` on the belief that the product fell
/// short; it does not, so the branch was defending against a number that was never wrong.
/// `0x3ff0000000000000` is `1.0`'s encoding, and the value was measured in `ge-python-oracle`:
///
/// ```text
/// >>> 65535.0 * (1.0 / 65535) == 1.0
/// True
/// ```
#[test]
fn the_grid_last_point_is_the_product_not_a_fix_up() {
    let step = super::super::grid_step();
    let product = 65535.0 * step;
    assert_eq!(bits(product), 0x3ff0_0000_0000_0000, "1.0, exactly");
    assert_eq!(product, 1.0);
    assert_eq!(
        super::super::grid_at(65535, step),
        product,
        "grid_at is the plain product, with no last-element branch"
    );
}
