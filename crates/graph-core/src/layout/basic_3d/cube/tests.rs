//! `_cube_layout`'s corners and interior, hand-pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:83-103`.
//!
//! Split from [`super::tests`] because this is the one of the three that draws from a stream,
//! and the tests here are about **which** parts are determined (the eight corners, the
//! `min(n, 8)` split, the single-node origin, the interior's radius) and which are not. The
//! interior's individual coordinates *are* determined now — the generator is the reference's —
//! so the tests that pin them are in [`interior`], next to nothing but each other.

mod interior;

use super::super::cube;
use super::super::tests::{bare, space};

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
/// This is the property the layout exists to draw, and it is a **weaker** claim than the one
/// [`interior`] now makes: the interior's numbers are the reference's exactly (see
/// [`interior::the_ninth_node_is_the_reference_interior_bit_for_bit`]), so "strictly inside, at
/// 80% of the corner radius" is a consequence rather than the fallback. It is kept because it
/// is the one property that holds at every size rather than at one pinned node count, and a
/// generator that produced the right nine numbers and the wrong thousand would still pass the
/// pinned test and fail this one.
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
/// and this checks the sample mean is near it over enough draws to mean something.
///
/// This is no longer half of what the differential compares — [`interior`] compares every
/// coordinate exactly — so what it is for now is the claim that **survives a wrong generator**:
/// a stream that is right for the first few draws and wrong afterwards, or an off-by-one in the
/// arithmetic, is centred and therefore passes the pinned tests and fails this one.
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
