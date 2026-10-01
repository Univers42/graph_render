//! The 3D arm at the LAYOUT level: that the arms differ, that `run_at(_, 2)` is the 2D
//! layout byte for byte, that a component above the dense limit solves at the reference's
//! 5-column LOBPCG block, and that a 3-node component solves rather than falling back to
//! the reference's random layout.
//!
//! The packing lattice and the dims-dependent geometry are tested where they now live, in
//! `spectral/space/tests.rs`. The 2D spectral/pivot-MDS behaviour is already covered in
//! `tests.rs` and `tests/end_to_end.rs`; what is new here is that `dims` is a *parameter*,
//! so the property worth pinning is that the 2D call is still the 2D call.

use super::*;
use crate::index::index_model;
use crate::layout::coords::probe::graph;

#[test]
fn a_run_at_two_dimensions_is_byte_identical_to_the_two_d_layout() {
    // The load-bearing property of this whole change: parameterising `dims` must not
    // have moved a single 2D byte.
    let g = graph(
        12,
        &[(0, 1), (1, 2), (3, 4), (5, 6), (7, 8), (9, 10), (2, 3)],
    );
    let (via_default, _) = run(&g).expect("solves");
    let (via_two, _) = run_at(&g, 2).expect("solves");
    assert_eq!(
        via_default, via_two,
        "run and run_at(_, 2) must be one layout"
    );
    let (mds_default, _) = crate::layout::pivot_mds::run(&g).expect("solves");
    let (mds_two, _) = crate::layout::pivot_mds::run_at(&g, 2).expect("solves");
    assert_eq!(mds_default, mds_two);
}

#[test]
fn the_three_d_run_differs_from_the_two_d_one_in_every_column() {
    let g = graph(
        12,
        &[(0, 1), (1, 2), (3, 4), (5, 6), (7, 8), (9, 10), (2, 3)],
    );
    let (flat, _) = run(&g).expect("solves");
    let (spaced, _) = run_at(&g, 3).expect("solves");
    assert_eq!(flat.dim(), graph_contract::snapshot::Dim::D2);
    assert_eq!(spaced.dim(), graph_contract::snapshot::Dim::D3);
    assert_ne!(
        flat.nodes, spaced.nodes,
        "a third eigenpair moves x and y too"
    );
    assert!(spaced.z.is_some_and(|z| z.iter().all(|v| v.is_finite())));
}

#[test]
fn the_wider_lobpcg_block_converges_on_the_first_fixture_above_the_dense_limit() {
    // Found by the 1000-seed spectral differential, which refused to emit at seed 255:
    // the first gate model above DENSE_EIG_LIMIT (n = 257) is where the 3D arm takes the
    // LOBPCG branch, and with the reference's own `k = min(dims + 2, n - 1)` = 5 columns
    // it did not converge, so `run_at(_, 3)` returned NothingSolved on 45 consecutive
    // seeds while `run_at(_, 2)` (4 columns) passed all of them.
    //
    // This pins the fixture and the convergence, so a future block-size or MAXITER change
    // cannot quietly re-break the 3D arm above 256 nodes.
    use crate::linalg::dense_sym::eigh;
    use crate::linalg::lobpcg::lobpcg_smallest;
    use crate::stage::{gate_node_count, seeded_model};
    use crate::weights::REFERENCE_DEGREE;

    let seed = 255u32;
    let n = gate_node_count(seed) as usize;
    assert!(
        n > DENSE_EIG_LIMIT,
        "the fixture must be on the LOBPCG branch, n = {n}"
    );
    let (nodes, edges) = seeded_model(seed, n as u32, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    let neighbors = simple_neighbors(&topology);
    let members: Vec<u32> = (0..n as u32).collect();
    let graph = graph::ComponentGraph::build(&members, &neighbors, n);

    let out = lobpcg_smallest(
        |x: &[f64], y: &mut [f64]| graph.matvec(x, y),
        &graph.degree,
        graph.size(),
        DIMS_3D + 2,
    );
    assert!(
        out.iterations < 1500,
        "a {}-column block on n={n} exhausted the cap at {} iterations",
        DIMS_3D + 2,
        out.iterations
    );
    // And the three kept eigenpairs are the Laplacian's smallest NON-TRIVIAL ones, which
    // the dense solve below can answer exactly at this size. LOBPCG is handed `Y = 1` as
    // an exclusion vector, so a returned 0 is the Laplacian's trivial null space leaking
    // back in — the failure the residual gate is there to catch.
    let dense = eigh(&graph.dense_matrix(), graph.size());
    for d in 0..DIMS_3D {
        let got = out.eig.values[d];
        let want = dense.values[1 + d];
        let rel = (got - want).abs() / want.abs().max(1.0);
        assert!(rel < 1e-2, "eigenvalue {d}: got {got} want {want}");
    }
}

#[test]
fn a_three_node_component_solves_rather_than_falling_back_to_random() {
    // The reference returns _random_layout for n < 4 (networkx_layouts.py:257-258); this
    // motor refuses instead (C12), so a 3-node graph must SOLVE -- dims_eff = 2
    // non-trivial eigenpairs with the third column filled by scatter's copy rule.
    let g = graph(3, &[(0, 1), (1, 2)]);
    let (geometry, reports) = run_at(&g, 3).expect("a 3-node path must solve, not fall back");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved, "the single component solved");
    let z = geometry.z.expect("3D");
    assert_eq!(z.len(), 3);
    assert!(z.iter().all(|v| v.is_finite()));
}
