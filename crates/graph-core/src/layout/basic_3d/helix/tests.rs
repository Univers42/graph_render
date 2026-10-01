//! `_helix_layout`'s double helix, hand-pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:65-81`.
//!
//! Split from [`super::tests`] because these are all about one thing — the `levels == 1`
//! branch and the strand arithmetic — and that one thing is where a port fails silently.

use super::super::helix;
use super::super::tests::{bare, space};

/// `radius = scale * 0.3` (`basic.py:77`) = `1.5`, the fixed radius of every node.
const R: f64 = 1.5;

/// The one case the reference guards and this port must guard identically
/// (`basic.py:71-74`): `levels == 1` returns `t = 0.5` for **every** node.
///
/// `levels = (n + 1) // 2` is 1 at both `n = 1` and `n = 2`, so the branch covers two
/// sizes, not one. And `0.5` is the helix's **midpoint**: `z = 0.5*10 - 5 = 0`, not the
/// `-5` foot a `t = 0` fallback would give. A port that returned `t = 0` here would have
/// every formula still reading correctly and would draw a one-node helix at the bottom of
/// the curve.
#[test]
fn the_degenerate_branch_is_the_midpoint_at_both_sizes_it_covers() {
    for n in [1u32, 2] {
        let (_, _, z) = space(&helix(&bare(n)).expect("runs"));
        assert!(
            z.iter().all(|&v| v == 0.0),
            "n={n}: t = 0.5 puts every node at z = 0, not at the -scale foot"
        );
    }
    // And explicitly not the foot:
    let (_, _, foot) = space(&helix(&bare(1)).expect("runs"));
    assert_ne!(foot[0], -5.0_f32, "t = 0 would have given -scale");
}

/// `n = 2` is the same degenerate branch, so the two nodes are `pi` apart in angle —
/// antipodal, at the same height. **This is the row a port that guarded on `n == 1` dies
/// on**, because it divides by `levels - 1 == 0` here.
#[test]
fn the_two_node_helix_is_two_antipodal_points_at_the_midpoint() {
    let (x, y, _) = space(&helix(&bare(2)).expect("runs"));
    assert_eq!(
        (x[0], x[1]),
        (R as f32, -(R as f32)),
        "angle pi apart: cos, -cos"
    );
    assert!(
        y.iter().all(|v| f64::from(*v).abs() < 1e-6),
        "both on the x axis"
    );
}

/// `n = 4`: `levels = 2`, `t = (i // 2) / 1`, so `t = 0, 0, 1, 1` — two turns, two nodes
/// each, and `z = -5, -5, +5, +5` from `t*scale*2 - scale` (`basic.py:81`).
#[test]
fn four_nodes_are_two_turns_at_both_ends_of_the_z_axis() {
    let (x, _, z) = space(&helix(&bare(4)).expect("runs"));
    assert_eq!(z, vec![-5.0_f32, -5.0, 5.0, 5.0], "t = 0, 0, 1, 1");
    assert_eq!(
        (x[0], x[1]),
        (R as f32, -(R as f32)),
        "the foot pair, pi apart"
    );
    // t = 1 gives angle `4*pi`, which is the same angle as 0 — so the head pair is the
    // foot pair again, a whole turn up. This is the check that `4*pi` is there and not
    // `2*pi`: at `2*pi` the head pair would be rotated by pi and `x[2]` would be `-R`.
    assert_eq!((x[2], x[3]), (R as f32, -(R as f32)), "4*pi is 0 mod 2*pi");
}

/// **The odd node count leaves the extra node on strand 0** (`basic.py:66-68`): `n = 3`
/// gives `levels = 2`, so `t = 0, 0, 1` and node 2 — even, hence `(i % 2) == 0` — is on
/// strand 0. Nothing rounds to make the two strands even.
#[test]
fn the_odd_extra_node_rides_strand_zero() {
    let (x, _, z) = space(&helix(&bare(3)).expect("runs"));
    assert_eq!(z, vec![-5.0_f32, -5.0, 5.0], "node 2 is on the t = 1 turn");
    // Strand 0 at t = 0 and at t = 1 have the same angle (0 and 4*pi), so the extra node
    // sits directly above node 0 — the drawing is a helix and not two flat rings.
    assert_eq!(
        (x[0], x[2]),
        (R as f32, R as f32),
        "strand 0, foot and head"
    );
    assert_eq!(x[1], -(R as f32), "node 1 is strand 1, a half turn away");
}

/// The strand offset is exactly `pi` (`basic.py:76`), so consecutive nodes of a turn are
/// antipodal — over a size where many turns are drawn.
#[test]
fn each_turn_is_two_antipodal_nodes() {
    for n in [4u32, 9, 33] {
        let (x, y, _) = space(&helix(&bare(n)).expect("runs"));
        for turn_index in 0..(n as usize).div_ceil(2) {
            let (a, b) = (2 * turn_index, 2 * turn_index + 1);
            if b >= n as usize {
                break;
            }
            assert!(
                (f64::from(x[a]) + f64::from(x[b])).abs() < 1e-5
                    && (f64::from(y[a]) + f64::from(y[b])).abs() < 1e-5,
                "n={n} turn {turn_index} is not antipodal"
            );
        }
    }
}

/// The radius is `scale * 0.3` at **every** node and every `t` (`basic.py:77`) — a
/// constant, not a taper. A version that tapered would still look like a helix.
#[test]
fn the_radius_is_one_hundred_thirty_percent_of_nothing_in_particular() {
    for n in [1u32, 2, 4, 9, 20, 101] {
        let (x, y, _) = space(&helix(&bare(n)).expect("runs"));
        for i in 0..n as usize {
            let r = libm::sqrt(f64::from(x[i]).powi(2) + f64::from(y[i]).powi(2));
            assert!((r - R).abs() < 1e-5, "n={n} node {i} at {r}, not {R}");
        }
    }
}

/// `z = t*scale*2 - scale` runs exactly `[-scale, +scale]` and nothing wider, at every
/// size above the degenerate branch.
#[test]
fn z_spans_the_scale_exactly_and_stays_inside_it() {
    let (_, _, z) = space(&helix(&bare(17)).expect("runs"));
    assert_eq!(
        z.first().copied(),
        Some(-5.0_f32),
        "t = 0 is the -scale foot"
    );
    assert_eq!(z.last().copied(), Some(5.0_f32), "t = 1 is the +scale head");
    assert!(z.iter().all(|&v| (-5.0..=5.0).contains(&v)));
}

/// `t` is monotone across the whole helix, so the drawing never doubles back on itself in
/// z — the property that makes it a helix rather than a scribble.
#[test]
fn t_is_monotone_in_the_node_index() {
    let (_, _, z) = space(&helix(&bare(24)).expect("runs"));
    for pair in z.windows(2) {
        assert!(pair[1] >= pair[0], "z must not go backwards");
    }
}

/// The f64 columns and their narrowing: the cast is the only step after the reference's
/// arithmetic.
#[test]
fn the_narrowed_columns_are_the_f64_columns_rounded_once() {
    let (x, y, z) = super::columns(7);
    let narrowed = space(&helix(&bare(7)).expect("runs"));
    for i in 0..7usize {
        assert_eq!(narrowed.0[i], x[i] as f32);
        assert_eq!(narrowed.1[i], y[i] as f32);
        assert_eq!(narrowed.2[i], z[i] as f32);
    }
}

#[test]
fn the_layout_is_named_by_its_module() {
    assert_eq!(super::ID, "layout.basic3d.helix");
}
