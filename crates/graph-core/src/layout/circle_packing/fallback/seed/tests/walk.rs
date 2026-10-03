//! The walk itself: what a seeded Fruchterman–Reingold run does to a graph, when it
//! stops, and that a step with no temperature is a step that moves nothing.

use super::super::{
    FrField, THRESHOLD, adjacency_matrix, fruchterman_reingold, initial_temperature, seed_positions,
};
use super::bits;

#[test]
fn a_cooled_step_moves_every_node_and_reports_the_whole_move() {
    // The step returns the Frobenius norm of `delta_pos` — over the whole matrix, not the
    // largest single node — and the temperature falls by the same `dt` every iteration
    // (`drawing/layout.py:717`).
    let n = 5u32;
    let edges = [(0, 1), (1, 2), (2, 3), (3, 4)];
    let adjacency = adjacency_matrix(n, &edges);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(1.0 / f64::from(n)),
    };
    let mut pos = seed_positions(n);
    let t = initial_temperature(&pos);
    let moved = field.step(&mut pos, t);
    // Every node moved, and none moved by more than the cap `t`.
    let before = seed_positions(n);
    for (i, (now, was)) in pos.iter().zip(&before).enumerate() {
        assert_ne!(*now, *was, "node {i} did not move");
        let d = libm::hypot(now.0 - was.0, now.1 - was.1);
        assert!(
            d <= t * (1.0 + 1e-9),
            "node {i} moved {d} against a cap of {t}"
        );
    }
    // The norm is over all the nodes' moves squared, and it is non-zero here.
    let recomputed: f64 = pos
        .iter()
        .zip(&before)
        .map(|((x, y), (bx, by))| {
            let (dx, dy) = (x - bx, y - by);
            dx * dx + dy * dy
        })
        .sum();
    assert!((moved - f64::sqrt(recomputed)).abs() < 1e-12, "{moved}");
}

#[test]
fn a_zero_temperature_step_moves_nothing() {
    // The cap scales by `t / length`, so a zero temperature is a zero move whatever the
    // displacement is — and the walk still reports how far it would have gone.
    let n = 4u32;
    let adjacency = adjacency_matrix(n, &[(0, 1), (2, 3)]);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(0.25),
    };
    let mut pos = seed_positions(n);
    let before = pos.clone();
    let moved = field.step(&mut pos, 0.0);
    assert_eq!(bits(&pos), bits(&before), "nothing moved");
    assert_eq!(moved, 0.0);
}

#[test]
fn the_walk_pulls_a_connected_graph_together_and_is_reproducible() {
    // The behaviour the seed exists for: after a full seeded walk, an edge's endpoints
    // are closer together than two non-adjacent nodes are, and two runs agree bit for
    // bit. (Exact tangency is the relaxation's job, not the seed's.)
    let n = 8u32;
    let mut edges = Vec::new();
    for i in 0..n - 1 {
        edges.push((i, i + 1));
    }
    edges.push((0, n - 1));
    let a = fruchterman_reingold(n, &edges, 50, None);
    let b = fruchterman_reingold(n, &edges, 50, None);
    assert_eq!(bits(&a), bits(&b), "same input, same bits");
    let spread = |i: u32, j: u32| {
        libm::hypot(
            a[i as usize].0 - a[j as usize].0,
            a[i as usize].1 - a[j as usize].1,
        )
    };
    let edge_mean: f64 = edges.iter().map(|&(i, j)| spread(i, j)).sum::<f64>() / edges.len() as f64;
    let far = spread(0, 4);
    assert!(
        edge_mean < far,
        "edges {edge_mean} shorter than a diameter of {far}"
    );
    assert!(a.iter().all(|&(x, y)| x.is_finite() && y.is_finite()));
}

#[test]
fn a_graph_with_no_edges_is_left_as_the_seed_and_the_walk_spread_it_out() {
    // No springs, only repulsion: the seed's own spiral becomes the packing, and nothing
    // can collapse or blow up.
    let pos = fruchterman_reingold(5, &[], 50, None);
    assert_eq!(pos.len(), 5);
    assert!(pos.iter().all(|&(x, y)| x.is_finite() && y.is_finite()));
    for (i, &(x, y)) in pos.iter().enumerate() {
        for (j, &(bx, by)) in pos.iter().enumerate() {
            if i == j {
                continue;
            }
            let d = libm::hypot(x - bx, y - by);
            assert!(d > 0.0, "nodes {i} and {j} coincide at {d}");
        }
    }
}

#[test]
fn the_threshold_stops_the_walk_once_the_average_move_is_negligible() {
    // The reference's own convergence test (`drawing/layout.py:718-719`): the Frobenius
    // norm of the last step's moves, over the node count, against `1e-4`. The check is on
    // the step's own norm, not on the count, so a packing that has already stopped moving
    // breaks out after one round however large the budget.
    let n = 4u32;
    let edges = [(0, 1), (1, 2), (2, 3)];
    let adjacency = adjacency_matrix(n, &edges);
    let field = FrField {
        adjacency: &adjacency,
        n,
        k: f64::sqrt(0.25),
    };
    let mut pos = [(0.0, 0.0); 4];
    let mut temperature = 1.0_f64;
    let dt = temperature / 101.0;
    let mut rounds = 0;
    for _ in 0..100 {
        let moved = field.step(&mut pos, temperature);
        temperature -= dt;
        rounds += 1;
        if moved / f64::from(n) < THRESHOLD {
            break;
        }
    }
    assert_eq!(rounds, 1, "a settled packing must stop at once");
    // And a live one keeps going: the same loop over the spiral seed has not settled
    // after a single round.
    let mut pos = seed_positions(n);
    let moved = field.step(&mut pos, 1.0);
    assert!(moved / f64::from(n) >= THRESHOLD, "{moved}");
    assert_eq!(THRESHOLD, 1e-4, "the reference's own threshold");
}
