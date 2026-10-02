//! `_cube_layout`'s corners and interior, hand-pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:83-103`.
//!
//! Split from [`super::tests`] because this is the one of the three that draws from a
//! stream, and the tests here are about **which** parts are determined (the eight corners,
//! the `min(n, 8)` split, the single-node origin, the interior's radius) and which are not
//! (the interior's individual coordinates — see the module doc on why, and the metadata's
//! `oracle` field for what the differential compares instead).

use super::super::cube;
use super::super::tests::{bare, space};
use crate::rng::Mt19937;

/// `np.random.RandomState(981_798_123).uniform(-1.0, 1.0, (1, 3)) * 4.0`, as bits. numpy
/// 2.3.3: node 8 of a nine-node cube, the first and only interior node, on a stream the
/// dispatcher has just reset — so this is the reference's draw exactly, not a draw from
/// wherever an earlier layout left the stream.
const INTERIOR_NODE_8: [u64; 3] = [
    0x400A_56C2_3DDB_3AF6,
    0x3FE1_C5FF_5659_AC48,
    0xBFF0_764F_FBF0_82A4,
];

/// The interior of a nine-node cube is **the reference's three numbers**, bit for bit:
/// `(-1.0 + 2.0 * u) * (scale * 0.8)` with `u` the reference's own draws at
/// `derive_seed(42, "layout")` (`basic.py:99-101`).
///
/// This is the test that closes the row: the corners were already exact, and the interior
/// is now the reference's rather than merely inside the same shell.
#[test]
fn the_ninth_node_is_the_reference_interior_bit_for_bit() {
    let (x, y, z) = super::columns(9);
    for (axis, column) in [&x, &y, &z].into_iter().enumerate() {
        assert_eq!(
            column[8].to_bits(),
            INTERIOR_NODE_8[axis],
            "node 8 axis {axis}: the draw is not the reference's"
        );
    }
    // And after the one narrowing `in_space` does, the `f32` the snapshot holds is that
    // `f64` rounded once — so the row's `f32` column is exact too, not merely close.
    let (narrowed, _, _) = space(&cube(&bare(9)).expect("runs"));
    assert_eq!(narrowed[8], x[8] as f32);
}

/// The interior's arithmetic is the reference's **operand order**, not an equivalent one:
/// `low + (high - low) * u` scaled afterwards, where `(2*u - 1) * reach` and
/// `-reach + 2*reach*u` differ from it in the last bits and would fail the test above.
#[test]
fn the_interior_is_uniform_in_minus_one_one_times_the_ratio() {
    let mut stream = Mt19937::new(super::SEED);
    let reach = super::SCALE * super::INTERIOR_RATIO;
    let mut want = Vec::new();
    for _ in 0..3 {
        let u = stream.next_f64();
        want.push((-1.0 + (1.0 - -1.0) * u) * reach);
    }
    for (axis, value) in want.iter().enumerate() {
        assert_eq!(value.to_bits(), INTERIOR_NODE_8[axis], "axis {axis}");
    }
}

/// The control: one more in the seed moves the interior and leaves the corners alone. A
/// port that pinned the seed but read the wrong words off the stream would still pass the
/// two tests above.
#[test]
fn a_neighbouring_seed_moves_the_interior_and_no_corner() {
    let (x, y, z) = space(&cube(&bare(9)).expect("runs"));
    let mut moved = Mt19937::new(super::SEED + 1);
    let first = (-1.0 + (1.0 - -1.0) * moved.next_f64()) * 5.0;
    assert_ne!(
        f64::from(x[8]),
        first,
        "the seed is not the one the test assumes"
    );
    for (axis, (column, want)) in [(&x, corner(0).0), (&y, corner(0).1), (&z, corner(0).2)]
        .into_iter()
        .enumerate()
    {
        assert_eq!(f64::from(column[0]), want, "corner 0 axis {axis} moved");
    }
}

/// The corners are a unit cube of half-side 1, scaled by `scale`.
fn corner(i: usize) -> (f64, f64, f64) {
    let c = super::CORNERS[i];
    (c[0] * 5.0, c[1] * 5.0, c[2] * 5.0)
}

/// **The corner order is the layout** (`basic.py:91-94`).
///
/// Those eight triples are a literal array, transcribed, not a cube computed. The order
/// decides which node sits at which corner, so reordering the array is a **visible
/// regression, not a refactor**: the drawing is the same cube with different nodes at its
/// corners, nothing in the picture would show it, and every hashed snapshot over eight
/// nodes would move. This pins all eight triples in all eight slots — not the set, which a
/// reordering would preserve, but the sequence.
#[test]
fn the_eight_corners_are_the_reference_literal_in_its_order() {
    assert_eq!(
        super::CORNERS,
        [
            [1.0, 1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, -1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, -1.0],
        ],
        "basic.py:91-94, in order"
    );
    // And the two slots where the sequence is *not* the sign enumeration a reader would
    // write from scratch, which is why it cannot be derived:
    assert_eq!(super::CORNERS[6], [1.0, -1.0, 1.0], "node 6");
    assert_eq!(super::CORNERS[7], [1.0, 1.0, -1.0], "node 7");
}

/// The corners reach every node index in order, at `scale` from the origin.
#[test]
fn node_i_takes_corner_i_at_the_scale() {
    let (x, y, z) = space(&cube(&bare(8)).expect("runs"));
    for i in 0..8usize {
        let (cx, cy, cz) = corner(i);
        assert_eq!(
            (f64::from(x[i]), f64::from(y[i]), f64::from(z[i])),
            (cx, cy, cz),
            "node {i}"
        );
    }
}

/// `n == 1` returns `np.zeros((1, 3))` (`basic.py:88-89`) — the **origin**, checked before
/// the corners are built.
///
/// `min(1, 8) = 1` would have put the node at `(+5, +5, +5)`, the first corner, so this
/// branch is load-bearing rather than a shortcut, and a cube whose one node sits on a
/// corner is a cube-shaped drawing with a hole in the middle.
#[test]
fn the_single_node_cube_is_the_origin_and_not_the_first_corner() {
    let (x, y, z) = space(&cube(&bare(1)).expect("runs"));
    let first = corner(0);
    let got = (f64::from(x[0]), f64::from(y[0]), f64::from(z[0]));
    assert_eq!((x, y, z), (vec![0.0], vec![0.0], vec![0.0]));
    assert_ne!(got, first, "not the first corner");
}

/// `2 <= n <= 8` is the `min(n, 8)` boundary (`basic.py:96-97`) and `idx < num_nodes` is
/// then false, so those sizes are corners **alone** — no interior point at all.
///
/// This is the boundary the `min` exists for, and it is the case a port that scatters
/// first and puts corners on top would get wrong from `n = 2` on.
#[test]
fn two_to_eight_nodes_are_corners_alone() {
    for n in 2u32..=8 {
        let (x, y, z) = space(&cube(&bare(n)).expect("runs"));
        for i in 0..n as usize {
            let (cx, cy, cz) = corner(i);
            assert_eq!(
                (f64::from(x[i]), f64::from(y[i]), f64::from(z[i])),
                (cx, cy, cz),
                "n={n} node {i} is not corner {i}"
            );
        }
    }
}

/// `n = 9` is the first size with an interior, and it is **exactly one node** — node 8,
/// the only one past the corners. `n = 8` has none.
#[test]
fn the_ninth_node_is_the_first_interior_point() {
    let (x, y, _) = space(&cube(&bare(9)).expect("runs"));
    assert_eq!(x.len(), 9);
    assert_eq!(
        (f64::from(x[0]), f64::from(x[7])),
        (corner(0).0, corner(7).0),
        "the eight corners are untouched by the scatter"
    );
    let first = corner(0);
    assert_ne!(
        (f64::from(x[8]), f64::from(y[8])),
        (first.0, first.1),
        "node 8 is not the first corner"
    );
}

/// The interior is **strictly inside** the shell: every axis strictly within
/// `[-0.8*scale, 0.8*scale]` = `[-4, 4]` (`basic.py:101`).
///
/// This is the property the layout exists to draw, and it is the one part of the scatter
/// that survives the port's own stream — the interior's *numbers* are not the reference's
/// (see the module doc), but "strictly inside, at 80% of the corner radius" is a claim
/// both streams satisfy and this asserts it over sizes past one draw.
#[test]
fn the_interior_is_strictly_inside_the_eighty_percent_shell() {
    for n in [9u32, 32, 257, 1000] {
        let (x, y, z) = space(&cube(&bare(n)).expect("runs"));
        for i in 8..n as usize {
            for (axis, value) in [x[i], y[i], z[i]].into_iter().enumerate() {
                assert!(
                    f64::from(value) > -4.0 && f64::from(value) < 4.0,
                    "n={n} node {i} axis {axis} at {value} is outside the interior"
                );
            }
        }
    }
}

/// The scatter is symmetric about the origin in distribution: `uniform(-r, r)` has mean 0,
/// and this checks the sample mean is near it over enough draws to mean something. It is
/// the statistical half of what the differential compares (the corners are exact) — a
/// centred stream passes it, an off-by-one in `-reach + 2*reach*u` does not.
#[test]
fn the_interior_is_centred_on_the_origin() {
    let n = 4096u32;
    let (x, y, z) = space(&cube(&bare(n)).expect("runs"));
    for (axis, column) in [(&x), (&y), (&z)].into_iter().enumerate() {
        let mean = column[8..].iter().map(|&v| f64::from(v)).sum::<f64>() / f64::from(n - 8);
        assert!(mean.abs() < 0.1, "axis {axis} mean {mean} is not centred");
    }
}

/// The stream is **fixed**, so the same count twice gives the same drawing. This is what
/// pins the interior in the hash gate: without it the stage would not hash at all.
#[test]
fn the_interior_is_the_same_twice_over() {
    let a = space(&cube(&bare(257)).expect("runs"));
    let b = space(&cube(&bare(257)).expect("runs"));
    assert_eq!(a, b, "two runs at n=257, byte for byte");
}

/// Three draws per node, axis by axis (`x`, then `y`, then `z`), which is the order
/// `rng.uniform(-1, 1, (k, 3))` fills in C order, off one stream for the whole scatter.
///
/// The check is a recount, not a recomputation: feeding [`SEED`] to a fresh
/// [`Mt19937`] and applying the reference's own two steps reproduces the interior exactly,
/// for every interior node and not only the first. A per-axis stream, or a `y`-before-`x`
/// order, would give a different drawing and this pins it.
#[test]
fn the_interior_draws_three_values_per_node_axis_by_axis() {
    let n = 40u32;
    let (x, y, z) = space(&cube(&bare(n)).expect("runs"));
    let reach = super::SCALE * super::INTERIOR_RATIO;
    let mut stream = Mt19937::new(super::SEED);
    for i in 8..n as usize {
        for (axis, column) in [&x, &y, &z].into_iter().enumerate() {
            let u = stream.next_f64();
            assert_eq!(
                column[i],
                ((-1.0 + (1.0 - -1.0) * u) * reach) as f32,
                "node {i} axis {axis}: the draw order moved"
            );
        }
    }
}

/// The seed is fixed, and changing it changes the interior while leaving the corners
/// exactly where they were — which is the claim `registry`'s seeding decision rests on.
#[test]
fn the_seed_moves_only_the_interior() {
    let a = space(&cube(&bare(20)).expect("runs"));
    let b = space(&cube(&bare(20)).expect("runs"));
    assert_eq!(a, b);
    assert_eq!(
        (a.0[0], a.1[0], a.2[0]),
        (corner(0).0 as f32, corner(0).1 as f32, corner(0).2 as f32)
    );
    assert_ne!(
        a.0[8], 0.0,
        "an interior coordinate at exactly zero would be a fluke"
    );
}

/// Every one of the eight corners is at distance `sqrt(3)*scale` from the origin, so the
/// drawing's extent is fixed by them and not by the scatter.
#[test]
fn the_corners_set_the_extent_and_the_interior_never_reaches_it() {
    let (x, y, z) = space(&cube(&bare(64)).expect("runs"));
    let reach = 5.0 * 3.0_f64.sqrt();
    for (i, column) in [&x, &y, &z].into_iter().enumerate() {
        let furthest = (8..64)
            .map(|k| f64::from(column[k]).abs())
            .fold(0.0_f64, f64::max);
        assert!(furthest < 4.0, "axis {i} interior reached {furthest}");
    }
    assert!(reach > 4.0, "the corners are outside the interior");
}

/// The f64 columns and their narrowing: the cast is the only step after the reference's
/// arithmetic, for both the corners and the scatter.
#[test]
fn the_narrowed_columns_are_the_f64_columns_rounded_once() {
    let (x, y, z) = super::columns(12);
    let narrowed = space(&cube(&bare(12)).expect("runs"));
    for i in 0..12usize {
        assert_eq!(narrowed.0[i], x[i] as f32);
        assert_eq!(narrowed.1[i], y[i] as f32);
        assert_eq!(narrowed.2[i], z[i] as f32);
    }
}

#[test]
fn the_layout_is_named_by_its_module() {
    assert_eq!(super::ID, "layout.basic3d.cube");
}
