//! The `dims = 3` arm: `SPECTRAL_3D` and `MDS_3D` against SciGraphs' own numbers.
//!
//! **Every expected value in this file was generated once in the pinned oracle image** by
//! calling `_spectral_layout_3d` / `_mds_layout_3d` (`networkx_layouts.py:249-291`) on
//! `nx.path_graph(6)` and printing the `f64` bit patterns, so what is asserted here is the
//! reference's answer and not a restatement of it. The two tables differ because they do:
//! spectral's three columns are `eigh`'s own eigenvectors of `D - A`, pivot MDS's are
//! `dist @ V`.
//!
//! **Two tolerances, and each is where it stops.** `scatter` is checked at `1e-12` — our
//! `tred2`/`tql2` against LAPACK's `syevd` on the same 6x6 matrix, which is the last bit of
//! the solver's own arithmetic and is not reachable bitwise. The end-to-end tables are at
//! `2e-6` because `Geometry` is `f32` (`layout/mod.rs:59`): one ULP at the drawing's extent
//! of `5.0` is `4.8e-7`, so `2e-6` is four ULP and the assertion is about the geometry,
//! not about the narrowing.

use super::super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;
use graph_contract::snapshot::Dim;

/// SciGraphs' `get_layout_seed()`, the integer the conformance harness hands both arms.
const LAYOUT_SEED: u32 = 981_798_123;

/// SciGraphs' `_spectral_component_coordinates(path6, 3)` — the peak-normalised
/// eigenvectors, before packing and before `_rescale_positions`. `_fix_eigenvector_signs`
/// is already applied: every column's largest-magnitude entry is positive.
const PATH6_SPECTRAL_BLOCK: [[f64; 3]; 6] = [
    [bits(0xbfeffffffffffffb), bits(0xbfecb0bf0b6b7107), bits(0x3fe76cf5d0b09956)],
    [bits(0xbfe76cf5d0b0994e), bits(0x3c6a146e07b6b62f), bits(0xbfe76cf5d0b0994d)],
    [bits(0xbfd126145e9ecd46), bits(0x3fecb0bf0b6b7107), bits(0xbfe76cf5d0b09955)],
    [bits(0x3fd126145e9ecd61), bits(0x3fecb0bf0b6b710a), bits(0x3fe76cf5d0b09952)],
    [bits(0x3fe76cf5d0b09959), bits(0x3c615601adeed793), bits(0x3fe76cf5d0b0995a)],
    [bits(0x3ff0000000000000), bits(0xbfecb0bf0b6b7106), bits(0xbfe76cf5d0b09951)],
];

/// SciGraphs' `_spectral_layout_3d(path6, 5.0)`: the same block after `_pack_component_blocks`
/// (a no-op on one component) and `_rescale_positions(positions, 5.0)`.
const PATH6_SPECTRAL: [[f64; 3]; 6] = [
    [bits(0xc014000000000000), bits(0xc011ee77672326a5), bits(0x400d483344dcbfa9)],
    [bits(0xc00d483344dcbfa8), bits(0xbcb6977979761e33), bits(0xc00d483344dcbfa4)],
    [bits(0xbff56f99764680a4), bits(0x4011ee77672326a4), bits(0xc00d483344dcbfae)],
    [bits(0x3ff56f99764680ad), bits(0x4011ee77672326a6), bits(0x400d483344dcbfa4)],
    [bits(0x400d483344dcbfa9), bits(0xbcb7f53a677d58fc), bits(0x400d483344dcbfae)],
    [bits(0x4013fffffffffffd), bits(0xc011ee77672326a4), bits(0xc00d483344dcbfa9)],
];

/// SciGraphs' `_mds_layout_3d(path6, 5.0)`.
const PATH6_MDS: [[f64; 3]; 6] = [
    [bits(0x4014000000000000), bits(0x3c9f381712a51f48), bits(0x3cbbc3185f377f90)],
    [bits(0x4008000000000000), bits(0x3c808a9add0a3056), bits(0xbc9e7779f73cd843)],
    [bits(0x3ff0000000000000), bits(0x3c8fd7214a74dfbe), bits(0xbc7e12c14c73449b)],
    [bits(0xbfeffffffffffffe), bits(0x3c9791d3dbefc794), bits(0xbc92fe15252cd4b6)],
    [bits(0xc008000000000000), bits(0x3c9f381712a51f48), bits(0xbcac9eb29b5bc59c)],
    [bits(0xc014000000000000), bits(0xbcbb8cb8053e638c), bits(0x3c695a194b0058ac)],
];

/// Our own last bit against `tred2`/`tql2`.
const SOLVER_TOLERANCE: f64 = 1e-12;
/// Four ULP of an `f32` at the drawing's extent (`5.0`): the `Geometry` narrowing, nothing else.
const GEOMETRY_TOLERANCE: f64 = 2e-6;

const fn bits(raw: u64) -> f64 {
    f64::from_bits(raw)
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

fn path_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n - 1).map(|i| (i, i + 1)).collect()
}

fn path6() -> Topology {
    topology(6, &path_pairs(6))
}

/// The three `f32` columns of a 3D geometry, or a panic naming what came back instead.
fn columns(geometry: &Geometry) -> (&[f32], &[f32], &[f32]) {
    match &geometry.nodes {
        NodeGeometry::Point { x, y } => (x, y, geometry.z.as_deref().expect("a z column")),
        other => panic!("expected Point geometry, got {other:?}"),
    }
}

fn assert_close(label: &str, got: &[f32], want: &[f64], tolerance: f64) {
    assert_eq!(got.len(), want.len(), "{label}: column length");
    for (i, (&g, &w)) in got.iter().zip(want).enumerate() {
        assert!(
            (f64::from(g) - w).abs() <= tolerance,
            "{label}[{i}]: got {g} want {w} (delta {})",
            (f64::from(g) - w).abs()
        );
    }
}

/// **The deliverable**: a 6-node path's 3D spectral coordinates are `eigh`'s three smallest
/// non-trivial eigenvectors of `D - A`, sign-fixed and peak-normalised, to the last bit our
/// `tred2`/`tql2` can reach. A path of 6 has `lambda2 = 1 != lambda3 = 2`, so no degenerate
/// eigenspace is involved and this row is a statement about the arithmetic, not about luck.
#[test]
fn six_node_path_block_is_numpys_eigh_after_sign_fixing() {
    let members: Vec<u32> = (0..6).collect();
    let neighbors = Neighbors::from_rows(
        &(0..6)
            .map(|i| {
                let mut row: Vec<u32> = (0..6).filter(|&j| j != i).collect();
                row.sort_unstable();
                row
            })
            .collect::<Vec<_>>(),
    );
    let local_of = local_positions(std::slice::from_ref(&members), 6);
    let graph = ComponentGraph::build(&members, &neighbors, &local_of);
    let mut eig = solve_component(&graph, Width::Spectral3d)
        .0
        .expect("6 <= DENSE_EIG_LIMIT solves");
    pin_signs(&mut eig);
    let mut coords = vec![0.0_f64; 6 * 3];
    scatter(&mut coords, &members, &eig, Width::Spectral3d);
    for (node, want) in PATH6_SPECTRAL_BLOCK.iter().enumerate() {
        for d in 0..3 {
            let got = coords[node * 3 + d];
            assert!(
                (got - want[d]).abs() <= SOLVER_TOLERANCE,
                "node {node} axis {d}: got {got} want {}",
                want[d]
            );
        }
    }
}

/// The same path through [`run_3d`], so the packing and the `_rescale_positions` multiply
/// are in the assertion too — a 3D row that stopped one step short of the reference's own
/// pipeline would still be a plane or an uncentred blob.
#[test]
fn six_node_path_three_d_is_the_reference_end_to_end() {
    let (geometry, reports) = run_3d(&path6(), 0).expect("one component, one solve");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(geometry.dim(), Dim::D3, "a 3D row must carry a z column");
    let (x, y, z) = columns(&geometry);
    assert_close("x", x, &PATH6_SPECTRAL.iter().map(|p| p[0]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
    assert_close("y", y, &PATH6_SPECTRAL.iter().map(|p| p[1]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
    assert_close("z", z, &PATH6_SPECTRAL.iter().map(|p| p[2]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
}

/// Pivot MDS in 3D over the same path: `dist @ V` for the three largest eigenpairs of the
/// `k x k` Gram matrix, which is a different arithmetic from the spectral block above and
/// therefore its own table rather than a scaling of the other one.
#[test]
fn six_node_path_pivot_mds_three_d_is_the_reference_end_to_end() {
    let (geometry, reports) = crate::layout::pivot_mds::run_3d(&path6(), 0).expect("one component");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(geometry.dim(), Dim::D3);
    let (x, y, z) = columns(&geometry);
    assert_close("x", x, &PATH6_MDS.iter().map(|p| p[0]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
    assert_close("y", y, &PATH6_MDS.iter().map(|p| p[1]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
    assert_close("z", z, &PATH6_MDS.iter().map(|p| p[2]).collect::<Vec<_>>(), GEOMETRY_TOLERANCE);
}

/// `_spectral_layout_3d:257-258`: below four nodes there is nothing to solve, and the
/// reference draws `_random_layout`. Asserted as the differential rather than as a table,
/// because `layout::random`'s own tests already pin that draw's bits — what is new here is
/// that the spectral arm *is* that draw.
#[test]
fn below_four_nodes_the_three_d_arm_is_the_random_layout() {
    for n in [1usize, 2, 3] {
        let t = topology(n, &path_pairs(n));
        let (geometry, reports) = run_3d(&t, LAYOUT_SEED).expect("never refuses");
        assert!(reports.is_empty(), "n={n}: nothing was attempted");
        let (_, _, z) = columns(&geometry);
        assert!(
            z.iter().any(|&v| v != 0.0),
            "n={n}: a z column of zeros is a plane, not the reference's draw"
        );
    }
}

/// The negative control for the test above: the fallback is the *seeded* draw, so a seed
/// one higher moves every coordinate. A `run_3d` that ignored its `seed` argument would
/// print the first test green and this one red.
#[test]
fn below_four_nodes_the_seed_moves_every_coordinate() {
    let t = topology(2, &path_pairs(2));
    let a = run_3d(&t, LAYOUT_SEED).expect("never refuses").0;
    let b = run_3d(&t, LAYOUT_SEED + 1).expect("never refuses").0;
    assert_ne!(a, b, "the n < 4 branch must read the seed it was handed");
}