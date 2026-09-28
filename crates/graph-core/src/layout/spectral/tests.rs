//! Closed-form spectra and tier-dispatch tests for `layout::spectral`'s per-component
//! machinery (`ComponentGraph`, `solve_component`) — this file exercises the new
//! gather-based matvec and dense/LOBPCG dispatch spectral.rs adds, not the already
//! extensively-tested `linalg` tier internals (`linalg::dense_sym::tests`,
//! `linalg::lobpcg::tests`, both cross-checked against a test-only cyclic Jacobi solver
//! and closed-form spectra already). End-to-end `run(&Topology)` behaviour (C6
//! adjacency, packing, the required `C_300` circle, determinism) is `tests/end_to_end.rs`.

use super::*;

fn undirected(n: usize, edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let mut neighbors = vec![Vec::new(); n];
    for &(a, b) in edges {
        neighbors[a as usize].push(b);
        neighbors[b as usize].push(a);
    }
    for row in &mut neighbors {
        row.sort_unstable();
        row.dedup();
    }
    neighbors
}

fn path_neighbors(n: usize) -> Vec<Vec<u32>> {
    undirected(
        n,
        &(0..n as u32 - 1).map(|i| (i, i + 1)).collect::<Vec<_>>(),
    )
}

fn cycle_neighbors(n: usize) -> Vec<Vec<u32>> {
    undirected(
        n,
        &(0..n as u32)
            .map(|i| (i, (i + 1) % n as u32))
            .collect::<Vec<_>>(),
    )
}

fn star_neighbors(n: usize) -> Vec<Vec<u32>> {
    undirected(n, &(1..n as u32).map(|i| (0, i)).collect::<Vec<_>>())
}

fn complete_neighbors(n: usize) -> Vec<Vec<u32>> {
    let mut edges = Vec::new();
    for i in 0..n as u32 {
        for j in (i + 1)..n as u32 {
            edges.push((i, j));
        }
    }
    undirected(n, &edges)
}

fn grid_neighbors(rows: usize, cols: usize) -> Vec<Vec<u32>> {
    let idx = |r: usize, c: usize| (r * cols + c) as u32;
    let mut edges = Vec::new();
    for r in 0..rows {
        for c in 0..cols {
            if c + 1 < cols {
                edges.push((idx(r, c), idx(r, c + 1)));
            }
            if r + 1 < rows {
                edges.push((idx(r, c), idx(r + 1, c)));
            }
        }
    }
    undirected(rows * cols, &edges)
}

fn path_spectrum(n: usize) -> Vec<f64> {
    (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(core::f64::consts::PI * k as f64 / n as f64))
        .collect()
}

fn cycle_spectrum(n: usize) -> Vec<f64> {
    let mut v: Vec<f64> = (0..n)
        .map(|k| 2.0 - 2.0 * libm::cos(2.0 * core::f64::consts::PI * k as f64 / n as f64))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

fn star_spectrum(n: usize) -> Vec<f64> {
    let mut v = vec![1.0; n];
    v[0] = 0.0;
    v[n - 1] = n as f64;
    v
}

fn complete_spectrum(n: usize) -> Vec<f64> {
    let mut v = vec![n as f64; n];
    v[0] = 0.0;
    v
}

fn grid_spectrum(rows: usize, cols: usize) -> Vec<f64> {
    let (pr, pc) = (path_spectrum(rows), path_spectrum(cols));
    let mut v: Vec<f64> = pr
        .iter()
        .flat_map(|&a| pc.iter().map(move |&b| a + b))
        .collect();
    v.sort_by(f64::total_cmp);
    v
}

/// Solves the single component `0..n` (built straight from `neighbors`, bypassing
/// `Topology`/`find_components` entirely) and checks its `dims_eff` smallest nontrivial
/// eigenvalues against the closed form. Both tiers are compared the same way: the dense
/// branch's `sub_block` already skips the trivial `values[0] == 0`, and LOBPCG never
/// finds it in the first place (the `Y = 1` constraint projects the constant vector out
/// before the first Rayleigh-Ritz, `docs/decisions/eigensolver.md`) — so `want[1..]` is
/// the right slice to compare against either tier's returned block.
fn check_spectrum(neighbors: Vec<Vec<u32>>, n: usize, want: &[f64], expected_tier: Tier) {
    let members: Vec<u32> = (0..n as u32).collect();
    let graph = ComponentGraph::build(&members, &neighbors, n);
    let dims_eff = DIMS.min(n - 1);
    let (solved, tier, iterations) = solve_component(&graph, dims_eff);
    assert_eq!(tier, expected_tier, "n={n}");
    let eig = solved.unwrap_or_else(|| {
        panic!("n={n} tier={tier:?} did not solve (lobpcg iterations={iterations:?})")
    });
    for (i, (&got, &want)) in eig.values.iter().zip(&want[1..1 + dims_eff]).enumerate() {
        let rel = (got - want).abs() / want.abs().max(1.0);
        assert!(rel < 1e-2, "n={n} eigenvalue {i}: got {got} want {want}");
    }
}

#[test]
fn path_spectra_match_the_closed_form_dense_and_lobpcg() {
    for n in [2usize, 3, 8, 40, 100, 256] {
        check_spectrum(path_neighbors(n), n, &path_spectrum(n), Tier::Dense);
    }
    for n in [300usize, 400] {
        check_spectrum(path_neighbors(n), n, &path_spectrum(n), Tier::Lobpcg);
    }
}

#[test]
fn cycle_spectra_match_the_closed_form_dense_and_lobpcg() {
    for n in [3usize, 8, 40, 100, 256] {
        check_spectrum(cycle_neighbors(n), n, &cycle_spectrum(n), Tier::Dense);
    }
    check_spectrum(
        cycle_neighbors(300),
        300,
        &cycle_spectrum(300),
        Tier::Lobpcg,
    );
}

#[test]
fn star_spectra_match_the_closed_form_dense_and_lobpcg() {
    for n in [4usize, 50, 256] {
        check_spectrum(star_neighbors(n), n, &star_spectrum(n), Tier::Dense);
    }
    check_spectrum(star_neighbors(300), 300, &star_spectrum(300), Tier::Lobpcg);
}

#[test]
fn complete_spectra_match_the_closed_form_dense_and_lobpcg() {
    for n in [3usize, 8, 50] {
        check_spectrum(complete_neighbors(n), n, &complete_spectrum(n), Tier::Dense);
    }
    check_spectrum(
        complete_neighbors(300),
        300,
        &complete_spectrum(300),
        Tier::Lobpcg,
    );
}

#[test]
fn grid_spectra_match_the_closed_form_dense_and_lobpcg() {
    check_spectrum(grid_neighbors(4, 4), 16, &grid_spectrum(4, 4), Tier::Dense);
    check_spectrum(
        grid_neighbors(16, 16),
        256,
        &grid_spectrum(16, 16),
        Tier::Dense,
    );
    check_spectrum(
        grid_neighbors(20, 20),
        400,
        &grid_spectrum(20, 20),
        Tier::Lobpcg,
    );
}

/// Pins down `nothing_solved`'s exact rule directly (a real solver failure is not
/// something our well-tested tiers fail on demand, so the logic is exercised as a pure
/// function instead of through a fabricated end-to-end failure): an error only when a
/// component of size `>= 2` was attempted and none solved; an all-singleton graph
/// attempts nothing and is not this error (`docs/decisions/eigensolver.md`, C12).
#[test]
fn nothing_solved_only_when_something_was_attempted_and_none_solved() {
    let singleton = |i: u32| vec![i];
    let pair = |a: u32, b: u32| vec![a, b];
    assert!(
        !nothing_solved(&[], false),
        "no components at all: nothing to attempt"
    );
    assert!(
        !nothing_solved(&[singleton(0), singleton(1)], false),
        "all singletons: nothing attempted"
    );
    assert!(
        nothing_solved(&[pair(0, 1)], false),
        "one attempt, it failed"
    );
    assert!(
        !nothing_solved(&[pair(0, 1)], true),
        "one attempt, it solved"
    );
    assert!(
        nothing_solved(&[singleton(0), pair(1, 2)], false),
        "a singleton plus one failed attempt"
    );
    assert!(
        !nothing_solved(&[singleton(0), pair(1, 2)], true),
        "a singleton plus one solved attempt"
    );
}

mod end_to_end;
