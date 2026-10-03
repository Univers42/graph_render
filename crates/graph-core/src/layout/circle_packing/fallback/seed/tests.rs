//! `seed`'s own arithmetic, checked against the reference it ports and against the
//! scheme it must *not* be: a step is a gather (D10) and is Jacobi, exactly as
//! networkx 3.6's vectorised `pos += delta_pos` is — every displacement is read from
//! the round's starting positions, and only then do the nodes move. The walk's own
//! cases are in [`walk`].
//!
//! [`walk`]: walk

mod walk;

use super::{
    FrField, GOLDEN_ANGLE, THRESHOLD, adjacency_matrix, fruchterman_reingold, initial_temperature,
    rescale_to, start_positions,
};

/// The in-place (Gauss–Seidel) step `step` had before the Jacobi fix: node `i` is moved
/// before node `i + 1` reads it. Test-only reference for the difference, never production
/// code.
fn gauss_seidel_step(field: &FrField<'_>, pos: &mut [(f64, f64)], t: f64) -> f64 {
    let mut moved_sq = 0.0;
    for i in 0..field.n as usize {
        let d = field.displacement(pos, i);
        let len = libm::hypot(d.0, d.1).max(0.01);
        let (dx, dy) = (d.0 * t / len, d.1 * t / len);
        pos[i].0 += dx;
        pos[i].1 += dy;
        moved_sq += dx * dx + dy * dy;
    }
    f64::sqrt(moved_sq)
}

/// The whole seeded walk, in the Gauss–Seidel scheme, so a test can hold the port's own
/// `fruchterman_reingold` against a scheme it must differ from.
fn gauss_seidel(n: u32, edges: &[(u32, u32)], iterations: u32) -> Vec<(f64, f64)> {
    let adjacency = adjacency_matrix(n, edges);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(1.0 / f64::from(n.max(1))),
    };
    let mut pos = start_positions(n, None);
    let mut t = initial_temperature(&pos);
    let dt = t / f64::from(iterations + 1);
    for _ in 0..iterations {
        let moved = gauss_seidel_step(&field, &mut pos, t);
        t -= dt;
        if moved / f64::from(n.max(1)) < THRESHOLD {
            break;
        }
    }
    pos
}

pub(super) fn bits(positions: &[(f64, f64)]) -> Vec<u64> {
    positions
        .iter()
        .flat_map(|&(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

#[test]
fn a_step_is_jacobi_so_no_node_sees_another_nodes_move() {
    // Three nodes in a path: the smallest graph on which the two schemes can differ at
    // all (with two or fewer nodes there is nothing for a later node to read).
    let edges = [(0, 1), (1, 2)];
    let jacobi = fruchterman_reingold(3, &edges, 10, None);
    let sequential = gauss_seidel(3, &edges, 10);
    assert_ne!(
        bits(&jacobi),
        bits(&sequential),
        "this case must separate the two schemes, or the test proves nothing"
    );
}

#[test]
fn a_displacement_is_repulsion_from_everyone_less_attraction_along_the_edges() {
    // The reference's own force (`drawing/layout.py:703-705`):
    // `sum_j (pos_i - pos_j) * (k^2 / d^2 - A[i][j] * d / k)`, over every `j != i`, in
    // ascending `j` (D3). With `A[i][j] = 1` the two terms cancel at `k^2 / d^2 = d / k`,
    // i.e. at `d = k` — the fixed point the layout relaxes toward.
    let n = 2;
    let k: f64 = f64::sqrt(0.5); // k = sqrt(1 / n)
    let adjacency = adjacency_matrix(n, &[(0, 1)]);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k,
    };
    let at_rest = [(0.0, 0.0), (k, 0.0)];
    assert_eq!(
        field.displacement(&at_rest, 0),
        (0.0, 0.0),
        "repulsion cancels attraction at d = k"
    );

    // Closer than that, repulsion wins and the node is pushed away from its neighbour.
    let close = [(0.0, 0.0), (k * 0.5, 0.0)];
    let pushed = field.displacement(&close, 0);
    assert!(pushed.0 < 0.0, "pushed left: {pushed:?}");
    // Further, attraction wins and the node is pulled toward it.
    let far = [(0.0, 0.0), (k * 4.0, 0.0)];
    let pulled = field.displacement(&far, 0);
    assert!(pulled.0 > 0.0, "pulled right: {pulled:?}");
    // The two ends of an edge feel equal and opposite forces. (The collinear `y` term is
    // `+0.0` at one end and `-0.0` at the other, so it is compared by value.)
    let other = field.displacement(&close, 1);
    assert_eq!(other.0.to_bits(), (-pushed.0).to_bits());
    assert_eq!(other.1, -pushed.1);
}

#[test]
fn a_displacement_ignores_the_nodes_own_slot() {
    // `j == i` contributes exactly zero (the difference vector is `(0, 0)`), so skipping
    // it — as the reference's matrix product does only by the same accident — is not a
    // behavioural change. Pinned so the skip stays a skip.
    let n = 3;
    let adjacency = adjacency_matrix(n, &[(0, 1)]);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(1.0 / 3.0),
    };
    let pos = [(0.5, -0.25), (1.5, 0.75), (-1.0, 2.0)];
    let want = field.displacement(&pos, 1);
    // By hand, over the two other nodes only.
    let k = field.k;
    let (mut fx, mut fy) = (0.0, 0.0);
    for j in 0..3usize {
        if j == 1 {
            continue;
        }
        let (dx, dy) = (pos[1].0 - pos[j].0, pos[1].1 - pos[j].1);
        let dist: f64 = libm::hypot(dx, dy).max(0.01);
        let factor = k * k / (dist * dist) - adjacency[3 + j] * dist / k;
        fx += dx * factor;
        fy += dy * factor;
    }
    assert_eq!(want.0.to_bits(), fx.to_bits());
    assert_eq!(want.1.to_bits(), fy.to_bits());
    // And the distance floor: two coincident nodes do not divide by zero.
    let coincident = [(0.0, 0.0), (0.0, 0.0), (0.0, 0.0)];
    let f = field.displacement(&coincident, 0);
    assert!(f.0.is_finite() && f.1.is_finite(), "{f:?}");
    assert_eq!(f, (0.0, 0.0), "no difference vector, no force");
}

#[test]
fn a_nearly_coincident_pair_is_measured_against_the_references_own_floor() {
    // The reference floors the distance at 0.01 (`np.clip(distance, 0.01, None)`,
    // `drawing/layout.py:701`) so the `k^2 / d^2` term can never divide by zero and the
    // repulsion can never exceed `k^2 / 0.0001`. Pinned at a separation *below* the floor,
    // where the floor is the whole answer: 0.005 is floored to 0.01, so the repulsive term
    // is `k^2 / 1e-4` and not `k^2 / 2.5e-5`.
    let n = 2u32;
    let k: f64 = f64::sqrt(0.5);
    let adjacency = adjacency_matrix(n, &[]);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k,
    };
    let close = [(0.0, 0.0), (0.005, 0.0)];
    let f = field.displacement(&close, 0);
    // dx = -0.005, d floored to 0.01, A = 0, so f.0 = -0.005 * k^2 / 1e-4.
    let want = -0.005 * (k * k / 1e-4);
    assert_eq!(f.0.to_bits(), want.to_bits(), "floored at 0.01: {f:?}");
    // Above the floor the true distance is used instead, and the two differ.
    let apart = [(0.0, 0.0), (0.5, 0.0)];
    let f = field.displacement(&apart, 0);
    let want = -0.5 * (k * k / (0.5 * 0.5));
    assert_eq!(f.0.to_bits(), want.to_bits(), "unfloored: {f:?}");
    assert_ne!(
        field.displacement(&close, 0).0.to_bits(),
        field.displacement(&apart, 0).0.to_bits()
    );
}

#[test]
fn an_adjacency_counts_a_multiplicity_and_is_symmetric() {
    // `nx.to_numpy_array` on a graph with parallel edges: the entry is the number of
    // edges between the pair, and it is the same in both directions.
    let a = adjacency_matrix(3, &[(0, 1), (0, 1), (1, 0), (1, 2)]);
    assert_eq!(a.len(), 9);
    assert_eq!(a[0], 0.0, "no self-loop");
    assert_eq!(a[1], 3.0, "three edges between 0 and 1");
    assert_eq!(a[3], 3.0, "and the same the other way");
    assert_eq!(a[5], 1.0);
    assert_eq!(a[2] + a[6], 0.0, "0 and 2 are not joined");
}

#[test]
fn the_seed_is_a_golden_spiral_filling_the_unit_disk() {
    // The reference's replacement for `seed.rand(n, dim)`: node `i` at radius
    // `sqrt((i + 1) / n)` and angle `i * GOLDEN_ANGLE`, so no two nodes share a radius
    // and none is at the origin.
    let n = 8;
    let pos = start_positions(n, None);
    assert_eq!(pos.len(), n as usize);
    let mut radii: Vec<f64> = pos.iter().map(|&(x, y)| libm::hypot(x, y)).collect();
    radii.sort_by(f64::total_cmp);
    assert!(
        radii.windows(2).all(|w| w[0] < w[1]),
        "strictly outward: {radii:?}"
    );
    assert!(radii[0] > 0.0, "the first node is not at the centre");
    assert!(radii[n as usize - 1] <= 1.0, "inside the unit disk");
    // Node `i`'s radius is exactly `sqrt((i + 1) / n)`, and its angle `i * GOLDEN_ANGLE`.
    for (i, &(x, y)) in pos.iter().enumerate() {
        let r = f64::sqrt(f64::from(i as u32 + 1) / f64::from(n));
        let a = f64::from(i as u32) * GOLDEN_ANGLE;
        assert_eq!(x.to_bits(), (r * libm::cos(a)).to_bits(), "node {i} x");
        assert_eq!(y.to_bits(), (r * libm::sin(a)).to_bits(), "node {i} y");
    }
    // The golden angle is the one the interactive force layout already uses, and it is
    // irrational enough that no small prefix repeats a direction.
    assert!((GOLDEN_ANGLE - 2.399_963_229_728_653).abs() < 1e-15);
    assert_ne!(pos[0], pos[1]);
}

#[test]
fn the_initial_temperature_is_a_tenth_of_the_widest_axis() {
    // The reference's own `t` (`drawing/layout.py:687`):
    // `max(x-span, y-span) * 0.1`.
    let pos = [(-1.0, 0.0), (1.0, 0.0), (0.0, 5.0)];
    assert_eq!(
        initial_temperature(&pos).to_bits(),
        0.5_f64.to_bits(),
        "y spans 5"
    );
    // Whichever axis spans more, and only that one: here x spans 9 and y only 4.
    let wide = [(0.0, -3.0), (9.0, 1.0)];
    assert_eq!(
        initial_temperature(&wide).to_bits(),
        0.9_f64.to_bits(),
        "x spans 9"
    );
    // A single node has no span, so the reference's own fold gives zero. (An empty
    // packing is not a case the reference reaches either: it indexes `pos.T[0]`.)
    assert_eq!(initial_temperature(&[(4.0, 4.0)]), 0.0);
}

#[test]
fn the_optimal_distance_is_the_reference_sqrt_one_over_n() {
    // `k = np.sqrt(1.0 / nnodes)` when `k is None` (`drawing/layout.py:682-683`).
    for n in [2u32, 3, 7, 16, 100] {
        let edges = [(0, 1)];
        let adjacency = adjacency_matrix(n, &edges);
        let field = FrField {
            adjacency: &adjacency,
            n,
            k: f64::sqrt(1.0 / f64::from(n)),
        };
        let want = 1.0 / f64::sqrt(f64::from(n));
        assert!((field.k - want).abs() < 1e-15, "n = {n}");
        // Wider spacing for a bigger graph, which is the point of the `1 / n`.
        if n > 2 {
            assert!(field.k < f64::sqrt(0.5), "n = {n}: k shrinks");
        }
    }
}

#[test]
fn rescale_to_centres_then_scales_to_the_frame() {
    let mut positions = [(-1.0, 2.0), (3.0, -4.0), (5.0, 0.0)];
    rescale_to(&mut positions, 2.0);
    // Mean-centred: the mean of the input is (7/3, -2/3), removed before scaling, so the
    // result is mean-zero and its largest magnitude on either axis is exactly 2.0.
    let mean_x = positions.iter().map(|p| p.0).sum::<f64>() / 3.0;
    let mean_y = positions.iter().map(|p| p.1).sum::<f64>() / 3.0;
    assert!(mean_x.abs() < 1e-12, "x mean {mean_x}");
    assert!(mean_y.abs() < 1e-12, "y mean {mean_y}");
    let max = positions
        .iter()
        .fold(0.0_f64, |m, &(x, y)| m.max(x.abs()).max(y.abs()));
    assert_eq!(max.to_bits(), 2.0_f64.to_bits());
    // And the aspect ratio is preserved, which is `rescale_layout`'s own contract.
    let before: Vec<(f64, f64)> = [(-1.0, 2.0), (3.0, -4.0), (5.0, 0.0)]
        .iter()
        .map(|&(x, y)| (x - 7.0 / 3.0, y + 2.0 / 3.0))
        .collect();
    let ratio_before = (before[2].0 - before[0].0) / (before[2].1 - before[0].1);
    let ratio_after = (positions[2].0 - positions[0].0) / (positions[2].1 - positions[0].1);
    assert!(
        (ratio_before - ratio_after).abs() < 1e-12,
        "{ratio_before} vs {ratio_after}"
    );
}

#[test]
fn rescaling_a_degenerate_all_zero_packing_leaves_it_at_the_origin() {
    // The reference's own `if lim > 0` guard: a frame with no extent must not be scaled
    // by a division by zero.
    let mut positions = [(0.0, 0.0), (0.0, 0.0)];
    rescale_to(&mut positions, 5.0);
    assert_eq!(positions, [(0.0, 0.0), (0.0, 0.0)]);
    let mut empty: [(f64, f64); 0] = [];
    rescale_to(&mut empty, 5.0);
    assert!(empty.is_empty());
}
