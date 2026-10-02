//! `layout::pivot_mds::run` tests: a hand-verified closed-form embedding for the
//! smallest nontrivial case (`P_3`), determinism, multi-component C12 reporting, and
//! the expected pivot count including the `k = 100` cap.
//!
//! The `P_3` case is worked by hand: with `n = 3`, `k = min(100, 3) = 3`, so every node
//! becomes a pivot. Farthest-point selection picks local pivots `0, 2, 1` in that order
//! (traced against `pivot_hops`' own tie rule); squaring and double-centering the
//! resulting `3x3` hop-distance matrix gives the exact Gram matrix
//! `[[2,-2,0],[-2,2,0],[0,0,0]]`, whose eigenvalues are `{4, 0, 0}`. The eigenvalue-4
//! eigenvector is simple and projects (up to sign) to `(sqrt2, 0, -sqrt2)`; **both**
//! eigenvalue-0 eigenvectors project to exactly `(0, 0, 0)` regardless of which
//! orthonormal basis the solver picks for that degenerate subspace — `Gv = 0` implies
//! `‖Bv‖² = vᵀBᵀBv = vᵀGv = 0`, so `Bv = 0` for *any* `v` in `G`'s null space, not just
//! the one the solver happens to return. So the result is fully determined: after
//! sign-pinning (index 0 wins the `|sqrt2|` tie) and peak-normalisation, node 1 lands
//! exactly at the origin, symmetric between the two endpoints.

use super::*;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

fn topology(n: usize, pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| edge(&format!("e{k}"), &a.to_string(), &b.to_string()))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn path_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n - 1).map(|i| (i, i + 1)).collect()
}

fn cycle_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n).map(|i| (i, (i + 1) % n)).collect()
}

fn star_pairs(n: usize) -> Vec<(usize, usize)> {
    (1..n).map(|i| (0, i)).collect()
}

fn complete_pairs(n: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            v.push((i, j));
        }
    }
    v
}

fn grid_pairs(rows: usize, cols: usize) -> Vec<(usize, usize)> {
    let idx = |r: usize, c: usize| r * cols + c;
    let mut v = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                v.push((idx(r, c), idx(r, c + 1)));
            }
            if r + 1 < rows {
                v.push((idx(r, c), idx(r + 1, c)));
            }
        }
    }
    v
}

fn points(geometry: &Geometry) -> (&[f32], &[f32]) {
    match &geometry.nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("expected Point geometry, got {other:?}"),
    }
}

#[test]
fn a_three_node_path_lands_on_three_exactly_spaced_points() {
    let t = topology(3, &path_pairs(3));
    let (geometry, reports) = run(&t).expect("solves");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(reports[0].pivots, 3);

    let (x, y) = points(&geometry);
    let close = |got: f32, want: f32| (got - want).abs() < 1e-6;
    assert!(close(x[0], 1.0) && close(y[0], 0.0), "{:?}", (x[0], y[0]));
    assert!(close(x[1], 0.0) && close(y[1], 0.0), "{:?}", (x[1], y[1]));
    assert!(close(x[2], -1.0) && close(y[2], 0.0), "{:?}", (x[2], y[2]));
}

#[test]
fn is_deterministic_run_twice() {
    let t = topology(60, &grid_pairs(6, 10));
    assert_eq!(run(&t), run(&t), "same input bits, same output bits");
}

#[test]
fn common_shapes_solve_with_the_expected_pivot_count() {
    for (n, pairs) in [
        (30usize, path_pairs(30)),
        (30, cycle_pairs(30)),
        (30, star_pairs(30)),
        (20, complete_pairs(20)),
    ] {
        let t = topology(n, &pairs);
        let (_, reports) = run(&t).expect("solves");
        assert_eq!(reports.len(), 1, "n={n}");
        assert!(reports[0].solved, "n={n}");
        assert_eq!(
            reports[0].pivots, n as u32,
            "n={n}: pivots = min(100, n) = n"
        );
    }
    // Past MAX_PIVOTS, the pivot count caps at 100 rather than following n.
    let big = topology(150, &cycle_pairs(150));
    let (_, reports) = run(&big).expect("solves");
    assert_eq!(reports[0].pivots, MAX_PIVOTS as u32);
}

#[test]
fn disconnected_graph_reports_only_attempted_components() {
    let mut nodes: Vec<_> = (0..5).map(|i| node(&format!("p{i}"), "")).collect();
    nodes.push(node("iso", ""));
    nodes.extend((0..4).map(|i| node(&format!("c{i}"), "")));
    let mut edges: Vec<_> = (0..4)
        .map(|i| edge(&format!("pe{i}"), &format!("p{i}"), &format!("p{}", i + 1)))
        .collect();
    edges.extend((0..4).map(|i| {
        edge(
            &format!("ce{i}"),
            &format!("c{i}"),
            &format!("c{}", (i + 1) % 4),
        )
    }));
    let t = index_model(&nodes, &edges).expect("fits");

    let (geometry, reports) = run(&t).expect("both size >= 2 components solve");
    assert_eq!(
        reports.len(),
        2,
        "the isolated node gets no report: nothing was attempted for it"
    );
    assert!(reports.iter().all(|r| r.solved));
    assert_eq!(
        reports.iter().map(|r| r.size).collect::<Vec<_>>(),
        vec![5, 4]
    );

    let (x, y) = points(&geometry);
    assert_eq!(x.len(), 10);
    assert!(x.iter().chain(y).all(|v| v.is_finite()));
}

/// The selection as first ported: one BFS per pivot over the whole graph, gathered through
/// `members`, the minimum kept as `f64` with each chosen pivot marked `-1`.
fn reference_hops(neighbors: &Neighbors, members: &[u32], k: usize) -> Vec<u32> {
    let n = members.len();
    let (mut hops, mut covered, mut chosen) = (Vec::new(), vec![f64::INFINITY; n], 0);
    for _ in 0..k {
        let mut dist = vec![u32::MAX; neighbors.len()];
        dist[members[chosen] as usize] = 0;
        let mut queue = std::collections::VecDeque::from([members[chosen]]);
        while let Some(v) = queue.pop_front() {
            for &w in neighbors.row(v) {
                if dist[w as usize] == u32::MAX {
                    dist[w as usize] = dist[v as usize] + 1;
                    queue.push_back(w);
                }
            }
        }
        for (i, &g) in members.iter().enumerate() {
            hops.push(dist[g as usize]);
            covered[i] = covered[i].min(f64::from(dist[g as usize]));
        }
        covered[chosen] = -1.0;
        chosen = (0..n).fold(
            0,
            |best, i| if covered[i] > covered[best] { i } else { best },
        );
    }
    hops
}

#[test]
fn in_place_walks_choose_the_reference_pivots_and_hops() {
    let (grid, n) = (9 * 13, 9 * 13 + 150);
    let mut pairs = grid_pairs(9, 13);
    let mut state = 7_u32;
    for i in 0..300 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        pairs.push((grid + i % 150, grid + (state >> 8) as usize % 150));
    }
    let neighbors = simple_neighbors(&topology(n, &pairs));
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let mut checked = 0;
    for members in components.iter().filter(|m| m.len() > 1) {
        let k = MAX_PIVOTS.min(members.len());
        let local = neighbors.component(members, &local_of);
        assert_eq!(
            pivot_hops(&local, k),
            reference_hops(&neighbors, members, k)
        );
        checked += 1;
    }
    assert!(
        checked >= 2,
        "the grid and the random part are separate components"
    );
}
