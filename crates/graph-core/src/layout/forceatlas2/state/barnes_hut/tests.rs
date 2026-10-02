//! The tree repulsion against the dense pair loop it replaces, on the same state.

use super::super::{Fa2Params, Fa2State};
use super::{THETA2, Tree};
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};

/// `n` nodes, each linked to two earlier ones, so masses (degree + 1) vary.
fn graph(n: u32) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .flat_map(|i| [i / 2, (i * 7 + 3) % i])
        .enumerate()
        .map(|(e, j)| {
            let i = e as u32 / 2 + 1;
            edge(&format!("e{e}"), &format!("n{j}"), &format!("n{i}"))
        })
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// The repulsion alone on `state`'s current positions: dense when `theta2` is `None`.
fn repulsion(state: &mut Fa2State, theta2: Option<f64>) -> Vec<(f64, f64)> {
    state.ux.fill(0.0);
    state.uy.fill(0.0);
    match theta2 {
        Some(theta2) => state.repel_tree(&mut Tree::default(), theta2),
        None => state.repulsion(),
    }
    state
        .ux
        .iter()
        .copied()
        .zip(state.uy.iter().copied())
        .collect()
}

/// `(per-node rms of |approx - exact| / |exact|, total |approx - exact| / total |exact|)`.
/// The first inflates where a settled node's repulsion nearly cancels; the second is the
/// error on the force field as a whole.
fn errors(approx: &[(f64, f64)], exact: &[(f64, f64)]) -> (f64, f64) {
    let (mut node, mut diff, mut norm) = (0.0, 0.0, 0.0);
    for (&(ax, ay), &(ex, ey)) in approx.iter().zip(exact) {
        let (d2, e2) = (
            (ax - ex) * (ax - ex) + (ay - ey) * (ay - ey),
            ex * ex + ey * ey,
        );
        (node, diff, norm) = (node + d2 / e2, diff + d2, norm + e2);
    }
    (
        libm::sqrt(node / exact.len() as f64),
        libm::sqrt(diff / norm),
    )
}

/// A state partway into a dense run, where nodes have started to cluster.
fn settling(n: u32, iterations: u32) -> Fa2State {
    let params = Fa2Params {
        max_iter: iterations,
        ..Fa2Params::default()
    };
    let mut state = Fa2State::new(&graph(n), params);
    state.run();
    state
}

#[test]
fn a_closed_angle_sums_every_pair_exactly() {
    let mut state = settling(400, 5);
    let exact = repulsion(&mut state, None);
    let tree = repulsion(&mut state, Some(0.0));
    let (node, total) = errors(&tree, &exact);
    assert!(
        node < 1e-12 && total < 1e-12,
        "relative error {node:e} per node, {total:e} total"
    );
}

#[test]
fn a_far_cell_moves_the_force_by_a_few_percent_at_most() {
    for (n, iterations) in [(1500, 0), (1500, 30), (1500, 100), (5000, 100)] {
        let mut state = settling(n, iterations);
        let exact = repulsion(&mut state, None);
        let tree = repulsion(&mut state, Some(THETA2));
        let (node, total) = errors(&tree, &exact);
        assert!(
            node < 0.02,
            "n {n}, after {iterations}: relative error {node} per node"
        );
        assert!(
            total < 0.05,
            "n {n}, after {iterations}: relative error {total} in total"
        );
    }
}

#[test]
fn the_tree_run_is_the_same_twice_and_finite() {
    let run = |_| {
        let mut state = Fa2State::new(&graph(300), Fa2Params::default()).with_tree();
        state.run();
        let (x, y) = state.positions();
        (x.to_vec(), y.to_vec())
    };
    let (a, b) = (run(0), run(1));
    assert_eq!(a, b);
    assert!(a.0.iter().chain(&a.1).all(|v| v.is_finite()));
}
