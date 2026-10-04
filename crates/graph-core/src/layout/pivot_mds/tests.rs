//! `layout::pivot_mds::run` tests: a hand-verified closed-form embedding for the
//! smallest nontrivial case (`P_3`), **LF-11's hand-computed orientations for `C4` and
//! `C8`**, multi-component C12 reporting, and the expected pivot count including the
//! `k = 100` cap.
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
//!
//! **`P_3` cannot see LF-11 and the file says so.** Every tie there is at the eigenvalue
//! `0`, and a null-space direction projects to zero however it is rotated — that is why the
//! header can claim "fully determined" and why `is_deterministic_run_twice` over a 6x10 grid
//! (the old negative control, LF-26) proved nothing: it compared two runs of a *pure*
//! function and the grid it chose has its ties in the null space too. LF-11 is about a tie at
//! a **non-zero** eigenvalue, where the projection is `Bv ≠ 0` and the rotation reaches the
//! output. `c4_and_c8_land_on_their_hand_computed_orientation` is that fixture.
//!
//! The tests are split by topic into the child modules below: `canonical` holds LF-11,
//! `reporting` holds the `run`-level pins, `hops` holds the pivot-selection cross-check.
//! The helpers they share stay here.

use super::*;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

mod canonical;
mod hops;
mod reporting;

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
