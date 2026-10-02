//! The interior's **numbers**, as the reference drew them.
//!
//! Split from [`super`] because these four tests are the only ones here that assert a
//! *coordinate* rather than a property: they hold the layout against
//! `np.random.RandomState(981_798_123)`'s own output, bit for bit, which is what closes the
//! SciGraphs `CUBE` row (`docs/measurements/sg-mt19937.md`). The properties — inside the shell,
//! centred, fixed, three draws per node — stay next to the corners in [`super`].

use super::super::{INTERIOR_RATIO, SEED, columns};
use super::corner;
use crate::layout::basic_3d::tests::{bare, space};
use crate::layout::basic_3d::{SCALE, cube};
use crate::rng::Mt19937;

/// `np.random.RandomState(981_798_123).uniform(-1.0, 1.0, (1, 3)) * 4.0`, as bits. numpy
/// 2.3.3: node 8 of a nine-node cube, the first and only interior node, on a stream the
/// dispatcher has just reset (`dispatcher.py:22`) — so this is the reference's draw exactly,
/// not a draw from wherever an earlier layout left the stream.
const INTERIOR_NODE_8: [u64; 3] = [
    0x400A_56C2_3DDB_3AF6,
    0x3FE1_C5FF_5659_AC48,
    0xBFF0_764F_FBF0_82A4,
];

/// The interior of a nine-node cube is **the reference's three numbers**, bit for bit:
/// `(-1.0 + 2.0 * u) * (scale * 0.8)` with `u` the reference's own draws at
/// `derive_seed(42, "layout")` (`basic.py:99-101`).
///
/// This is the test that closes the row: the corners were already exact, and the interior is
/// now the reference's rather than merely inside the same shell.
#[test]
fn the_ninth_node_is_the_reference_interior_bit_for_bit() {
    let (x, y, z) = columns(9);
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
    let mut stream = Mt19937::new(SEED);
    let reach = SCALE * INTERIOR_RATIO;
    let mut want = Vec::new();
    for _ in 0..3 {
        let u = stream.next_f64();
        want.push((-1.0 + (1.0 - -1.0) * u) * reach);
    }
    for (axis, value) in want.iter().enumerate() {
        assert_eq!(value.to_bits(), INTERIOR_NODE_8[axis], "axis {axis}");
    }
}

/// The control: one more in the seed moves the interior and leaves the corners alone. A port
/// that pinned the seed but read the wrong words off the stream would still pass the two
/// tests above.
#[test]
fn a_neighbouring_seed_moves_the_interior_and_no_corner() {
    let (x, y, z) = space(&cube(&bare(9)).expect("runs"));
    let mut moved = Mt19937::new(SEED + 1);
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

/// Three draws per node, axis by axis (`x`, then `y`, then `z`), which is the order
/// `rng.uniform(-1, 1, (k, 3))` fills in C order, off one stream for the whole scatter.
///
/// The check is a recount, not a recomputation: feeding `SEED` to a fresh [`Mt19937`] and
/// applying the reference's own two steps reproduces the interior exactly, for every interior
/// node and not only the first. A per-axis stream, or a `y`-before-`x` order, would give a
/// different drawing and this pins it.
#[test]
fn the_interior_draws_three_values_per_node_axis_by_axis() {
    let n = 40u32;
    let (x, y, z) = space(&cube(&bare(n)).expect("runs"));
    let reach = SCALE * INTERIOR_RATIO;
    let mut stream = Mt19937::new(SEED);
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
