//! `layout::pivot_mds::run` tests: a hand-verified closed-form embedding for the
//! smallest nontrivial case (`P_3`), determinism, multi-component C12 reporting, and
//! the expected pivot count including the `k = 100` cap.
//!
//! The `P_3` case is worked by hand: with `n = 3`, `k = min(100, 3) = 3`, so every node
//! becomes a pivot. Farthest-point selection picks local pivots `0, 2, 1` in that order
//! (traced against `pivot_distances`' own tie rule); squaring and double-centering the
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
use super::matrix::SUM_ZERO;
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

/// `n x k` values from a fixed LCG with exact zeros of both signs mixed in. Column 0 is
/// positive and the last column all `-0.0`, so their dot product is a sum of `-0.0`s: the
/// one case where a sum's start shows in its bytes.
fn signed_zero_matrix(n: usize, k: usize) -> Vec<f64> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut cells = Vec::with_capacity(n * k);
    for i in 0..n * k {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let v = (state >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
        cells.push(match (i % k, state % 7) {
            (0, _) => 1.0 + v,
            (j, _) if j == k - 1 => -0.0,
            (_, 0) => -0.0,
            (_, 1) => 0.0,
            _ => v * 1e3,
        });
    }
    cells
}

#[test]
fn gram_is_the_column_dot_product_bit_for_bit() {
    assert_eq!(
        SUM_ZERO.to_bits(),
        std::iter::empty::<f64>().sum::<f64>().to_bits()
    );
    let (n, k) = (257, 13);
    let dist = signed_zero_matrix(n, k);
    let row_wise: Vec<u64> = gram(&dist, k).iter().map(|v| v.to_bits()).collect();
    let mut dot_products = Vec::with_capacity(k * k);
    for p in 0..k {
        for q in 0..k {
            let dot: f64 = (0..n).map(|i| dist[i * k + p] * dist[i * k + q]).sum();
            dot_products.push(dot.to_bits());
        }
    }
    assert_eq!(row_wise, dot_products);
}

#[test]
fn double_centering_keeps_the_column_means_bytes() {
    let (n, k) = (101, 7);
    let mut dist = signed_zero_matrix(n, k);
    let squared: Vec<f64> = dist.iter().map(|v| v * v).collect();
    double_center(&mut dist, n, k);
    let col = |j: usize| (0..n).map(|i| squared[i * k + j]).sum::<f64>() / n as f64;
    let row = |i: usize| (0..k).map(|j| squared[i * k + j]).sum::<f64>() / k as f64;
    let grand = (0..k).map(col).sum::<f64>() / k as f64;
    for i in 0..n {
        for j in 0..k {
            let expect = (squared[i * k + j] - col(j) - row(i) + grand) * -0.5;
            assert_eq!(dist[i * k + j].to_bits(), expect.to_bits(), "({i}, {j})");
        }
    }
}
