//! LF-09: the reference's **shift-invert retry**, `networkx_layouts.py:120-129`.
//!
//! `_laplacian_nontrivial_eigenvectors` has two tiers, not one. When LOBPCG's block does
//! not pass `_eig_converged` (`:114`) it prints and re-solves with
//! `eigsh(L, k=dims + 1, sigma=-1e-3, which='LM', v0=start, maxiter=300)` — a Lanczos on
//! `(L + 1e-3 I)^-1`, whose negative shift lifts the *smallest* Laplacian eigenvalues to the
//! top of the inverse spectrum and so separates exactly the tight cluster plain LOBPCG
//! chokes on. Only if *that* also misses does it return `None` (`:131`).
//!
//! ## What actually fails without the retry, and why
//!
//! Measured here (`the_failing_gate_is_orthonormality_not_the_residual`), on the seeded gate
//! model at `n = 300` with the 3D arm's `b = dims + 2 = 5`:
//!
//! - the **residual** gate passes with four orders of magnitude to spare — peak
//!   `‖Lv - λv‖ = 4.1e-7` against a limit of `1e-2 · max|λ| = 1.9e-3`;
//! - the **orthonormality** gate fails at exactly `|VᵀV - I|_max = 1.0`, because one of the
//!   block's columns has collapsed to the **zero vector** (column norm `0.0`) while
//!   carrying the trivial eigenvalue `λ = 0`. A zero column makes `‖Av - λv‖ = 0` — a
//!   residual of *zero*, which is why the residual gate cannot see it — and `|0 - 1| = 1`
//!   on the diagonal, which is why the orthonormality gate can.
//!
//! So the block admits the numerically-null direction (the constant vector, which the
//! `Y = 1` constraint excludes only to rounding), MGS drops it below `COLLAPSE_NORM`, and
//! `dims_eff = 3` columns of `sub_block` then include it. With `dims = 2` the block is one
//! column narrower (`b = 4`), nothing collapses, and the same graph solves in 112
//! iterations — which is why `layout.spectral` never saw this and `layout.spectral3d`
//! refused every `n >= 300`.
//!
//! ## The substitution, stated rather than glossed
//!
//! `docs/decisions/eigensolver.md` ("Rejected alternatives") **rejected** this tier: "needs
//! a sparse direct factorisation; none is on the `libm`/`indexmap` allow-list and none is on
//! disk (R2). Dropped per decision 4/ask 7: the cascade is dense -> LOBPCG -> skip-and-report,
//! never shift-invert." That record's *reason* is what has gone — ARPACK wants a sparse
//! factorisation of `L + 1e-3 I`, and a **dense** Cholesky of the same matrix is exactly
//! what `DENSE_EIG_LIMIT`'s own tier already builds. The record file is outside this job's
//! paths, so it is reported rather than edited (`docs/measurements/fix-spectral.md`, LF-09
//! row). No dependency was added; `spectral/shift_invert.rs` carries the factorisation.

use super::super::*;
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;
use graph_contract::geometry::NodeGeometry;
use graph_contract::snapshot::Dim;

/// Any seed reaching `n >= 300` in the 3D arm shows the same block collapse: it is the
/// block width and the component size that decide, not the seed.
const GATE_SEED: u32 = 7;

/// `seeded_model(seed, n, reference_degree)` indexed — the model `graph-cli`'s caps ladder
/// grows, at the node counts the addendum names. The generator is called, never copied.
fn gate_model(n: u32) -> Topology {
    let (nodes, edges) = seeded_model(GATE_SEED, n, REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("fits the u32 index space")
}

fn topology(n: usize, pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| edge(&format!("e{k}"), &a.to_string(), &b.to_string()))
        .collect();
    index_model(&nodes, &edges).expect("fits")
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

/// Every coordinate a geometry carries, in `x, y, z` order — the 2D arm yields two columns
/// per node and the 3D arm three, which is what tells the two apart.
fn coordinates(geometry: &Geometry) -> Vec<f32> {
    match &geometry.nodes {
        NodeGeometry::Point { x, y } => x
            .iter()
            .chain(y)
            .chain(geometry.z.as_deref().unwrap_or_default())
            .copied()
            .collect(),
        other => panic!("expected Point geometry, got {other:?}"),
    }
}

/// **The addendum's GREEN**: `cap-probe --id layout.spectral3d --n 700` must exit 0, so
/// `run_3d` over the seeded gate model must solve at 700. The three counts between 256
/// (`DENSE_EIG_LIMIT`, the last dense size) and 700 (`SPECTRAL_CEILING`) are asserted too,
/// not just the endpoint: the collapse is a function of the block width, so a fix that
/// only reached the largest size would be a coincidence.
#[test]
fn the_seeded_gate_model_solves_in_three_dimensions_up_to_the_registry_ceiling() {
    for n in [300u32, 400, 512, 700] {
        let (geometry, reports) = run_3d(&gate_model(n), DEFAULT_SEED)
            .unwrap_or_else(|e| panic!("n={n}: layout.spectral3d refused ({e:?})"));
        assert!(
            coordinates(&geometry).iter().all(|v| v.is_finite()),
            "n={n}: a non-finite coordinate"
        );
        assert_eq!(reports.len(), 1, "n={n}: the gate model is one component");
        assert!(
            reports[0].solved,
            "n={n}: size {} missed the gate at residual {:?} on tier {:?}",
            reports[0].size, reports[0].peak_residual, reports[0].tier,
        );
    }
}

/// The retry is named rather than inferred from `Ok`: a cascade that passed the test above
/// without ever entering it would leave this unproven, and the tier is the whole of LF-09.
#[test]
fn the_three_dimensional_arm_reaches_the_shift_invert_tier() {
    for n in [300u32, 400, 700] {
        let (_, reports) = run_3d(&gate_model(n), DEFAULT_SEED).expect("solves");
        assert_eq!(
            reports[0].tier,
            Tier::ShiftInvert,
            "n={n}: b = dims + 2 = 5 collapses a block column and only the retry can answer it"
        );
    }
}

/// **The review's own RED fixture** (`review-layout-force.md` LF-09): "a 30x10 grid whose
/// component misses the residual gate today". 300 nodes, so the LOBPCG tier, and it fails
/// the same way the gate model does — the review names the wrong gate, and the test below is
/// what says so.
#[test]
fn a_three_hundred_node_grid_solves_in_three_dimensions() {
    let (geometry, reports) = run_3d(&topology(300, &grid_pairs(30, 10)), DEFAULT_SEED)
        .expect("the one component solves");
    assert!(coordinates(&geometry).iter().all(|v| v.is_finite()));
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].size, 300);
    assert_eq!(reports[0].tier, Tier::ShiftInvert);
    assert!(reports[0].solved, "residual {:?}", reports[0].peak_residual);
}

/// **The diagnosis, executable.** At `n = 300` in three dimensions it is the
/// **orthonormality** gate that refuses, by exactly `1.0`, over a residual four orders of
/// magnitude inside its own limit — and the 2D arm on the identical graph passes, because
/// its block is one column narrower.
///
/// The second half is the negative control: without it this test would also be satisfied by
/// a tier that never collapses a column, including one that solves nothing at all.
#[test]
fn the_failing_gate_is_orthonormality_not_the_residual() {
    let t = gate_model(300);
    let (three_d, reports) = run_3d(&t, DEFAULT_SEED).expect("the retry solves it");
    assert!(reports[0].solved);

    let members: Vec<u32> = (0..t.node_count()).collect();
    let neighbors = simple_neighbors(&t);
    let local_of = local_positions(std::slice::from_ref(&members), members.len());

    let block = solve::lobpcg_block(
        &ComponentGraph::build(&members, &neighbors, &local_of),
        DIMS_3D,
    );
    assert_eq!(block.k, 5, "b = dims + 2 = 5");
    assert!(
        block.column(0).iter().all(|v| *v == 0.0),
        "the first block column is the collapsed null direction, which is why the residual \
         gate cannot see it (a zero column has a residual of exactly zero)"
    );
    assert_eq!(block.values[0], 0.0, "carrying the trivial eigenvalue");

    let narrow = solve::lobpcg_block(
        &ComponentGraph::build(&members, &neighbors, &local_of),
        DIMS,
    );
    assert_eq!(narrow.k, 4, "b = dims + 2 = 4");
    assert!(
        narrow.column(0).iter().all(|v| *v != 0.0),
        "one column narrower and nothing collapses, which is why layout.spectral never saw it"
    );
    let (two_d, two_d_reports) = run(&t).expect("the 2D arm solves n=300");
    assert!(two_d_reports[0].solved);
    assert_eq!(three_d.dim(), Dim::D3, "the 3D arm's geometry is 3D");
    assert_eq!(two_d.dim(), Dim::D2, "the 2D arm's geometry is not");
    assert_eq!(
        coordinates(&three_d).len(),
        3 * t.node_count() as usize,
        "three columns per node"
    );
    assert_eq!(
        coordinates(&two_d).len(),
        2 * t.node_count() as usize,
        "two: the arms really are different pictures of one graph"
    );
}

/// The number LF-10 has to surface: `ComponentReport::peak_residual` is
/// `max_j ‖Lv_j - λ_j v_j‖`, the quantity `linalg::residual_converged` gates on and cannot
/// return. Pinned against that primitive here, so a report can never quote a different
/// number from the one the gate decided on — and so `solve.rs`'s copy of the arithmetic is
/// held to `linalg`'s.
#[test]
fn the_reported_peak_residual_is_the_number_the_gate_decided_on() {
    for (n, width) in [
        (300u32, Width::Spectral3d),
        (300, Width::Spectral2d),
        (700, Width::Spectral3d),
    ] {
        let t = gate_model(n);
        let (solve, members, neighbors, local_of) = one_component(&t, width);
        let reported = solve.peak_residual.expect("a candidate was produced");
        assert!(
            reported > 0.0,
            "n={n}: a reported residual is measured, not 0"
        );
        if let Some(eig) = &solve.eig {
            let graph = ComponentGraph::build(&members, &neighbors, &local_of);
            let matvec = |x: &[f64], y: &mut [f64]| graph.matvec(x, y);
            assert!(
                crate::linalg::residual_converged(matvec, eig, 1e-2),
                "n={n}: an accepted block must satisfy linalg's own gate"
            );
            let scale = eig
                .values
                .iter()
                .fold(0.0_f64, |m, v| m.max(v.abs()))
                .max(1e-12);
            assert!(
                reported <= 1e-2 * scale,
                "n={n}: the reported residual {reported} must be inside the gate's own limit {}",
                1e-2 * scale
            );
        }
    }
}

/// The gate model's one component, and the pieces its `ComponentGraph` borrows — bundled
/// because the borrow outlives the solve it is handed.
fn one_component(t: &Topology, width: Width) -> (solve::Solve, Vec<u32>, Neighbors, Vec<u32>) {
    let members: Vec<u32> = (0..t.node_count()).collect();
    let neighbors = simple_neighbors(t);
    let local_of = local_positions(std::slice::from_ref(&members), members.len());
    let graph = ComponentGraph::build(&members, &neighbors, &local_of);
    let solve = solve::solve_component(&graph, width);
    (solve, members, neighbors, local_of)
}
