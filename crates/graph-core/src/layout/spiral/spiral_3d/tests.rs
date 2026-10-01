use super::{arc_spaced, default_turns, half_to_even, invert, speed};
use crate::layout::Geometry;
use crate::layout::coords::probe::graph;
use graph_contract::geometry::NodeGeometry;

fn columns(g: &Geometry) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    let z = g.z.clone().expect("a 3D layout carries a z column");
    (x.clone(), y.clone(), z)
}

#[test]
fn the_reference_turns_formula_is_max_two_rounded_half_to_even() {
    // max(2, round(sqrt(n / (0.75*pi)))) -- basic.py:41
    assert_eq!(default_turns(1), 2.0);
    assert_eq!(default_turns(2), 2.0);
    assert_eq!(default_turns(8), 2.0);
    assert_eq!(default_turns(100), 7.0);
    // round is Python's: a tie goes to the even neighbour, where f64::round would not.
    assert_eq!(half_to_even(0.5), 0.0);
    assert_eq!(half_to_even(1.5), 2.0);
    assert_eq!(half_to_even(2.5), 2.0);
    assert_eq!(half_to_even(3.5), 4.0);
}

#[test]
fn speed_is_the_reference_expression_and_grows_with_t() {
    let omega = 2.0 * std::f64::consts::PI * 2.0;
    // sqrt(0.5^2 + (0.5 (1+t) omega)^2 + 2^2) at t = 0 is sqrt(0.25 + omega^2/4 + 4)
    let at_zero = libm::sqrt(0.25 + (0.5 * omega) * (0.5 * omega) + 4.0);
    assert!((speed(0.0, omega) - at_zero).abs() < 1e-12);
    assert!(speed(1.0, omega) > speed(0.0, omega));
}

#[test]
fn the_arc_table_is_strictly_increasing_so_the_inverse_is_single_valued() {
    let omega = 2.0 * std::f64::consts::PI * 2.0;
    let table = super::arc_table(omega);
    assert_eq!(table.grid.len(), super::SAMPLES);
    assert_eq!(table.length.len(), super::SAMPLES);
    assert_eq!(
        table.length[0], 0.0,
        "the reference seeds length with a zero"
    );
    assert!(table.length[1] > 0.0, "the first cell is a real trapezoid");
    assert!(
        table.length.windows(2).all(|w| w[1] > w[0]),
        "a flat or falling cell would make np.interp's bracket ambiguous"
    );
    assert_eq!(table.grid[0], 0.0);
    assert_eq!(
        table.grid[super::SAMPLES - 1],
        1.0,
        "linspace pins its endpoint"
    );
    assert!(invert(&table, -1.0) == 0.0);
    assert!(invert(&table, f64::INFINITY) == 1.0);
}

#[test]
fn inverting_a_wanted_just_under_the_total_stays_inside_the_table() {
    // The last node's `wanted` is `total * (n-1) / (n-1)`, which can sit a few ulps below
    // `length[-1]`; the bracket must still be the final cell, not one past the end. This
    // is the panic a 1000-seed run found that a handful of hand-picked n did not.
    let table = super::arc_table(2.0 * std::f64::consts::PI * 2.0);
    let last = table.length.len() - 1;
    let total = table.length[last];
    for epsilon in [0.0_f64, f64::EPSILON, 1e-15, 1e-12] {
        let t = invert(&table, total - epsilon);
        assert!(
            t.is_finite() && (0.0..=1.0).contains(&t),
            "t {t} at eps {epsilon}"
        );
    }
    assert_eq!(
        invert(&table, total),
        1.0,
        "the total maps to the end of the curve"
    );
}

#[test]
fn the_grid_spans_the_unit_interval_with_linspace_s_own_spacing() {
    // `invert` interpolates on the grid rather than on `index * step`, because
    // `np.interp` does. Measured: the two agree to 1.1e-16 in t, but the reference's own
    // output differs from an index-based t by up to 1.5e-3 at n = 583 -- the endpoint
    // pinning below is what makes the grid and the index expression agree at all, so this
    // pins the shape the inversion depends on.
    let table = super::arc_table(1.0);
    let step = 1.0 / (super::SAMPLES as f64 - 1.0);
    assert_eq!(table.grid.len(), super::SAMPLES);
    assert_eq!(table.grid[0], 0.0);
    assert_eq!(
        table.grid[super::SAMPLES - 1],
        1.0,
        "linspace pins its endpoint"
    );
    assert!((table.grid[1] - step).abs() < 1e-18, "grid[1] is the step");
    assert!(
        table.grid.windows(2).all(|w| w[1] > w[0]),
        "the grid ascends"
    );
}

#[test]
fn the_ends_are_the_reference_ends_and_a_single_node_is_mid_arc() {
    let (x, y, z) = arc_spaced(6, 2.0);
    // t = 0: radius 0.5, angle 0, z = -1
    assert!((x[0] - 0.5).abs() < 1e-12 && y[0].abs() < 1e-12 && (z[0] + 1.0).abs() < 1e-12);
    // t = 1: radius 1.0, z = +1
    assert!((z[5] - 1.0).abs() < 1e-9, "z {}", z[5]);
    // n = 1 takes the reference's 0.5 * length[-1] branch, landing mid-arc (basic.py:55)
    let (sx, sy, sz) = arc_spaced(1, 2.0);
    assert_eq!((sx.len(), sy.len(), sz.len()), (1, 1, 1));
    assert!((0.0..1.0).contains(&sz[0]) && (sz[0] - 1.0).abs() > 1e-6);
}

#[test]
fn z_climbs_monotonically_so_the_cone_is_not_a_flat_spiral() {
    let (_, _, z) = arc_spaced(40, 3.0);
    assert!(
        z.windows(2).all(|w| w[1] > w[0]),
        "z must climb with arc position"
    );
    assert!(z[0] >= -1.0 - 1e-9 && z[39] <= 1.0 + 1e-9);
}

#[test]
fn the_registered_run_emits_a_z_column_the_2d_arm_does_not() {
    let three_d = super::run(&graph(5, &[(0, 1)])).expect("runs");
    assert!(three_d.z.is_some(), "layout.spiral.3d is a 3D layout");
    let (x, y, z) = columns(&three_d);
    assert_eq!((x.len(), y.len(), z.len()), (5, 5, 5));
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
    assert_eq!(super::run(&graph(0, &[])).unwrap().z, Some(vec![]));
}

#[test]
fn the_three_d_curve_is_not_the_two_d_one_at_another_dimension() {
    // The reason this is its own kernel (spiral.rs:3-5): node 0 of the reference's
    // conical curve is (0.5, 0, -1) -- radius scale*0.5 at t = 0 -- where networkx's 2D
    // spiral puts node 0 at radius 0. Checked on n > 1, because n = 1 takes the
    // reference's mid-arc branch and never visits t = 0.
    let g = super::run(&graph(8, &[(0, 1)])).unwrap();
    let (x, y, z) = columns(&g);
    assert!((x[0] - 0.5).abs() < 1e-6, "x {}", x[0]);
    assert!(y[0].abs() < 1e-6, "y {}", y[0]);
    assert!((z[0] + 1.0).abs() < 1e-6, "z {}", z[0]);
}
