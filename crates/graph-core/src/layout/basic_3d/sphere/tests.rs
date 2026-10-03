//! `_sphere_layout`'s Fibonacci construction, hand-pinned against
//! `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:22-34`.
//!
//! Split from [`super::tests`], which holds the assertions common to all three
//! placements, because the sphere's are all about *its* two constants and about nothing
//! else: there is no graph, no branch and no stream, so each test below is one constant
//! under test.

use super::super::sphere;
use super::super::tests::{bare, golden, space};
use crate::layout::Geometry;

/// The `+0.5` band midpoint, which is what keeps the poles from over-packing
/// (`basic.py:24-26`).
///
/// At `n = 2` the latitudes are `1 - 2*0.5/2 = 0.5` and `1 - 2*1.5/2 = -0.5`, so both
/// nodes are off the poles. At the band **edges** they would be `+1` and `-1`, and
/// `sqrt(1 - 1) = 0` would collapse both onto the poles — the over-packing the reference's
/// own docstring says the midpoint avoids.
#[test]
fn the_latitudes_are_band_midpoints_so_neither_node_lands_on_a_pole() {
    let (_, y, _) = space(&sphere(&bare(2)).expect("runs"));
    assert_eq!(y, vec![2.5_f32, -2.5], "1 - 2*(i+0.5)/2, times scale");
    // At the band midpoints the radius is `sqrt(1 - 0.25)`, so both nodes are well off
    // the axis. At the band EDGES the latitudes would be `+1` and `-1`, `sqrt(1 - 1) = 0`
    // and both nodes would collapse onto the poles — the over-packing the reference's own
    // docstring says the midpoint avoids. So the check is each node's distance from the
    // axis, `sqrt(x^2 + z^2)`, which is the radius and is 0 only at a pole.
    let (x, _, z) = space(&sphere(&bare(2)).expect("runs"));
    let radius = 5.0 * f64::sqrt(1.0 - 0.25);
    for i in 0..2usize {
        let got = f64::sqrt(f64::from(x[i]).powi(2) + f64::from(z[i]).powi(2));
        assert!(
            (got - radius).abs() < 1e-4,
            "node {i} is {got} from the axis, not {radius}: a pole would be 0"
        );
    }
}

/// `n = 1`: `y = 1 - 2*0.5/1 = 0`, `radius = sqrt(1) = 1`, `theta = 0` — so the single
/// node is at `(+scale, 0, 0)`, the **equator on the +x axis**. It is neither the origin
/// nor a pole, which is the band midpoint again: at `n = 1` the one band is the equator.
#[test]
fn the_single_node_is_the_equator_on_the_positive_x_axis() {
    let (x, y, z) = space(&sphere(&bare(1)).expect("runs"));
    assert_eq!((x, y, z), (vec![5.0], vec![0.0], vec![0.0]));
}

/// `theta = pi*(3 - sqrt(5))*i` (`basic.py:32`), node 3 as the one to check.
///
/// `3 * golden` is about 7.18 rad. A golden-ratio constant written as `1.6180339887`, or an
/// angle accumulated node by node, lands somewhere else — and the accumulated form fails
/// only from node 3 on, so a test that checks nodes 0 and 1 cannot see it.
#[test]
fn node_three_is_the_third_golden_angle_and_not_an_accumulation() {
    let (x, y, z) = space(&sphere(&bare(4)).expect("runs"));
    assert_eq!(y[3], -3.75_f32, "node 3 is on the last band of four");
    let radius = f64::sqrt(1.0 - 0.75 * 0.75) * 5.0;
    let theta = 3.0 * golden();
    assert!(
        (f64::from(x[3]) - radius * libm::cos(theta)).abs() < 1e-4,
        "x: {} vs {}",
        x[3],
        radius * libm::cos(theta)
    );
    assert!(
        (f64::from(z[3]) - radius * libm::sin(theta)).abs() < 1e-4,
        "z"
    );
    // The accumulated form, for contrast: summing three golden angles in a row lands a few
    // 1e-16 away — inside f32, so invisible in a snapshot, which is exactly why the
    // columns are computed at f64 and narrowed once at the end.
    let mut accumulated = 0.0_f64;
    for _ in 0..3 {
        accumulated += golden();
    }
    assert!(
        (accumulated - theta).abs() < 1e-12,
        "the difference is real in f64 even where f32 hides it"
    );
    assert_ne!(y[3], y[0], "and the node is not a copy of node 0");
}

/// Axis order is the reference's (`basic.py:34`): `x` is the **cosine** column and `z` the
/// **sine** one, with `y` the latitude between them. A transposition is a rotation of the
/// whole drawing — it looks correct in a viewer and moves every digest.
#[test]
fn x_holds_the_cosine_and_z_the_sine_not_the_other_way_round() {
    let (x, y, z) = space(&sphere(&bare(7)).expect("runs"));
    // Each node's radius is its own: `sqrt(1 - y^2) * scale`, from `basic.py:30`. Without
    // it the columns would not be cosines at all, and a reader checking the axis order
    // would be checking the wrong thing.
    for i in 0..7usize {
        let latitude = f64::from(y[i]) / 5.0;
        let radius = f64::sqrt(1.0 - latitude * latitude) * 5.0;
        let t = golden() * f64::from(i as u32);
        assert!(
            (f64::from(x[i]) - radius * libm::cos(t)).abs() < 1e-3,
            "node {i} x: {} vs {}",
            x[i],
            radius * libm::cos(t)
        );
        assert!(
            (f64::from(z[i]) - radius * libm::sin(t)).abs() < 1e-3,
            "node {i} z: {} vs {}",
            z[i],
            radius * libm::sin(t)
        );
    }
    // And the two are genuinely different columns, not a transposed copy of each other:
    // node 0 is at `theta = 0`, where the sine column is exactly 0 and the cosine
    // column carries the node's whole radius.
    assert!(f64::from(z[0]).abs() < 1e-6, "node 0 is at theta = 0");
    let first = f64::sqrt(1.0 - f64::from(y[0]).powi(2) / 25.0) * 5.0;
    assert!((f64::from(x[0]) - first).abs() < 1e-4, "x[0] is {first}");
}

/// Every node is on the shell of radius `scale`, in all three columns, over a size range
/// that crosses the counts where the drawing changes character.
#[test]
fn every_node_is_on_the_shell_across_sizes() {
    for n in [1u32, 2, 3, 5, 9, 17, 64, 257] {
        let (x, y, z) = space(&sphere(&bare(n)).expect("runs"));
        for i in 0..n as usize {
            let radius = f64::sqrt(
                f64::from(x[i]).powi(2) + f64::from(y[i]).powi(2) + f64::from(z[i]).powi(2),
            );
            assert!((radius - 5.0).abs() < 1e-3, "n={n} node {i} at {radius}");
        }
    }
}

/// No two nodes coincide, over a size where a band-edge version would put two on each pole
/// and a phi-only version would pair up.
#[test]
fn no_two_nodes_coincide() {
    let (x, y, z) = space(&sphere(&bare(64)).expect("runs"));
    let points: Vec<u32> = x.iter().chain(&y).chain(&z).map(|v| v.to_bits()).collect();
    let distinct = points
        .iter()
        .collect::<std::collections::HashSet<_>>()
        .len();
    assert_eq!(distinct, 192, "two of the 64 nodes landed on one point");
}

/// The latitudes are **strictly decreasing** and span the closed interval without reaching
/// either pole — which is `y = 1 - 2*(i+0.5)/n` stated as an order and a bound, and is
/// the property that makes the bands equal-area.
#[test]
fn the_latitudes_decrease_and_never_touch_a_pole() {
    for n in [2u32, 5, 33] {
        let (_, y, _) = space(&sphere(&bare(n)).expect("runs"));
        for pair in y.windows(2) {
            assert!(pair[1] < pair[0], "n={n}: latitudes must decrease");
        }
        assert!(
            y[0] < 5.0 && *y.last().expect("non-empty") > -5.0,
            "n={n}: no pole"
        );
    }
}

/// The f64 columns are handed back before narrowing, and narrowing is the **only** thing
/// that happens after them: an f32 cast cannot reorder or recompute anything.
#[test]
fn the_narrowed_columns_are_the_f64_columns_rounded_once() {
    let (x, y, z) = super::columns(6);
    let narrowed = space(&sphere(&bare(6)).expect("runs"));
    for i in 0..6usize {
        assert_eq!(narrowed.0[i], x[i] as f32);
        assert_eq!(narrowed.1[i], y[i] as f32);
        assert_eq!(narrowed.2[i], z[i] as f32);
    }
}

/// The geometry the id names, at the size the differential sweeps from.
#[test]
fn the_layout_is_named_by_its_module() {
    assert_eq!(super::ID, "layout.basic3d.sphere");
    let g: Geometry = sphere(&bare(3)).expect("runs");
    assert_eq!(g.dim(), graph_contract::snapshot::Dim::D3);
}
