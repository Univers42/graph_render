//! The `dims = 3` arm: `SPECTRAL_3D` and `MDS_3D` against SciGraphs' own numbers.
//!
//! **Every expected value in this file was generated once in the pinned oracle image** by
//! calling `_spectral_layout_3d` / `_mds_layout_3d` (`networkx_layouts.py:249-291`) on
//! `nx.path_graph(6)` and printing the `f64` bit patterns, so what is asserted here is the
//! reference's answer and not a restatement of it. The two tables differ because they do:
//! spectral's three columns are `eigh`'s own eigenvectors of `D - A`, pivot MDS's are
//! `dist @ V`.
//!
//! ## Why the comparison is per-column up to sign
//!
//! `_fix_eigenvector_signs` (`:66-72`) pins a column by flipping it when its
//! **largest-magnitude** entry is negative. On a path every eigenvector is symmetric or
//! antisymmetric about the midpoint, so several entries have the same magnitude *exactly*,
//! and which of them `argmax` names is decided by the solver's last bit. On this 6-path the
//! raw `lambda = 2` eigenvector is `[∓0.5, 0, ±0.5, ±0.5, 0, ∓0.5]` and LAPACK's `syevd`
//! breaks the tie one way while our `tred2`/`tql2` breaks it the other — with the eigenvalues
//! themselves in agreement, so this is **not** an eigenvalue degeneracy. What is compared
//! here is therefore each column up to its own sign, and
//! [`a_six_path_columns_have_a_tied_largest_magnitude_entry`] names the tie rather than
//! leaving it as a tolerance nobody can explain.
//!
//! A sign flip commutes exactly through `_rescale_positions` (a per-column mean and a
//! positive global divisor), which is why the end-to-end tables are compared the same way.
//!
//! **Two tolerances, and each is where it stops.** `1e-12` is the last bit of two different
//! eigensolvers on the same 6x6 matrix. `2e-6` is four ULP of an `f32` at the drawing's
//! extent of `5.0` — `Geometry` is `f32` (`layout/mod.rs:59`), one ULP at `5.0` being
//! `4.8e-7` — so that assertion is about the geometry and not about the narrowing.

use super::super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;
use graph_contract::snapshot::Dim;

/// SciGraphs' `get_layout_seed()`, the integer the conformance harness hands both arms.
const LAYOUT_SEED: u32 = 981_798_123;

/// SciGraphs' `_spectral_component_coordinates(path6, 3)` — the peak-normalised
/// eigenvectors, before packing and before `_rescale_positions`. `_fix_eigenvector_signs`
/// is already applied.
const PATH6_SPECTRAL_BLOCK: [[f64; 3]; 6] = [
    [
        bits(0xbfeffffffffffffb),
        bits(0xbfecb0bf0b6b7107),
        bits(0x3fe76cf5d0b09956),
    ],
    [
        bits(0xbfe76cf5d0b0994e),
        bits(0x3c6a146e07b6b62f),
        bits(0xbfe76cf5d0b0994d),
    ],
    [
        bits(0xbfd126145e9ecd46),
        bits(0x3fecb0bf0b6b7107),
        bits(0xbfe76cf5d0b09955),
    ],
    [
        bits(0x3fd126145e9ecd61),
        bits(0x3fecb0bf0b6b710a),
        bits(0x3fe76cf5d0b09952),
    ],
    [
        bits(0x3fe76cf5d0b09959),
        bits(0x3c615601adeed793),
        bits(0x3fe76cf5d0b0995a),
    ],
    [
        bits(0x3ff0000000000000),
        bits(0xbfecb0bf0b6b7106),
        bits(0xbfe76cf5d0b09951),
    ],
];

/// SciGraphs' `_spectral_layout_3d(path6, 5.0)`: the same block after `_pack_component_blocks`
/// (a no-op on one component) and `_rescale_positions(positions, 5.0)`.
const PATH6_SPECTRAL: [[f64; 3]; 6] = [
    [
        bits(0xc014000000000000),
        bits(0xc011ee77672326a5),
        bits(0x400d483344dcbfa9),
    ],
    [
        bits(0xc00d483344dcbfa8),
        bits(0xbcb6977979761e33),
        bits(0xc00d483344dcbfa4),
    ],
    [
        bits(0xbff56f99764680a4),
        bits(0x4011ee77672326a4),
        bits(0xc00d483344dcbfae),
    ],
    [
        bits(0x3ff56f99764680ad),
        bits(0x4011ee77672326a6),
        bits(0x400d483344dcbfa4),
    ],
    [
        bits(0x400d483344dcbfa9),
        bits(0xbcb7f53a677d58fc),
        bits(0x400d483344dcbfae),
    ],
    [
        bits(0x4013fffffffffffd),
        bits(0xc011ee77672326a4),
        bits(0xc00d483344dcbfa9),
    ],
];

/// SciGraphs' `_mds_layout_3d(path6, 5.0)`.
const PATH6_MDS: [[f64; 3]; 6] = [
    [
        bits(0x4014000000000000),
        bits(0x3c9f381712a51f48),
        bits(0x3cbbc3185f377f90),
    ],
    [
        bits(0x4008000000000000),
        bits(0x3c808a9add0a3056),
        bits(0xbc9e7779f73cd843),
    ],
    [
        bits(0x3ff0000000000000),
        bits(0x3c8fd7214a74dfbe),
        bits(0xbc7e12c14c73449b),
    ],
    [
        bits(0xbfeffffffffffffe),
        bits(0x3c9791d3dbefc794),
        bits(0xbc92fe15252cd4b6),
    ],
    [
        bits(0xc008000000000000),
        bits(0x3c9f381712a51f48),
        bits(0xbcac9eb29b5bc59c),
    ],
    [
        bits(0xc014000000000000),
        bits(0xbcbb8cb8053e638c),
        bits(0x3c695a194b0058ac),
    ],
];

/// Two different eigensolvers on one 6x6 matrix: where the arithmetic stops.
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

/// The adjacency of `nx.path_graph(n)`, the graph every expected table in this file was
/// generated on.
fn path_neighbors(n: usize) -> Vec<Vec<u32>> {
    let mut rows = vec![Vec::new(); n];
    for (a, b) in path_pairs(n) {
        rows[a].push(b as u32);
        rows[b].push(a as u32);
    }
    for row in &mut rows {
        row.sort_unstable();
    }
    rows
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

/// Column `d` of an `n x 3` expected table.
fn column(table: &[[f64; 3]], d: usize) -> Vec<f64> {
    table.iter().map(|row| row[d]).collect()
}

/// Asserts `got` equals `want` up to one sign, and returns which sign it took.
///
/// The sign is reported rather than assumed so a caller can also check the *rule*
/// afterwards: whichever way a column came out, the entry `_fix_eigenvector_signs` looked at
/// must now be non-negative.
fn assert_column_up_to_sign(label: &str, got: &[f64], want: &[f64], tolerance: f64) -> f64 {
    assert_eq!(got.len(), want.len(), "{label}: column length");
    let signed = got
        .iter()
        .zip(want)
        .map(|(&g, &w)| (g - w).abs())
        .fold(0.0_f64, f64::max);
    let flipped = got
        .iter()
        .zip(want)
        .map(|(&g, &w)| (g + w).abs())
        .fold(0.0_f64, f64::max);
    let sign = if signed <= flipped { 1.0 } else { -1.0 };
    assert!(
        signed.min(flipped) <= tolerance,
        "{label}: got {got:?} want {want:?} \
(signed {signed}, flipped {flipped}, tolerance {tolerance})"
    );
    sign
}

/// The largest-magnitude entry, and every index that attains it (first one wins, as
/// `np.abs(col).argmax()` does).
fn peak_entry(column: &[f64]) -> (usize, f64, Vec<usize>) {
    let peak = column.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let tied: Vec<usize> = column
        .iter()
        .enumerate()
        .filter(|(_, v)| v.abs() == peak)
        .map(|(i, _)| i)
        .collect();
    (tied[0], peak, tied)
}

/// **The deliverable**: a 6-node path's 3D spectral coordinates are `eigh`'s three smallest
/// non-trivial eigenvectors of `D - A`, sign-fixed and peak-normalised, to the last bit our
/// `tred2`/`tql2` can reach — up to the per-column sign, for the reason this file's header
/// gives. The peak is the reference's own: one `abs().max()` over the whole block, not one
/// per column (`:154-156`).
#[test]
fn six_node_path_block_is_numpys_eigh_after_sign_fixing() {
    let members: Vec<u32> = (0..6).collect();
    let neighbors = Neighbors::from_rows(&path_neighbors(6));
    let local_of = local_positions(std::slice::from_ref(&members), 6);
    let graph = ComponentGraph::build(&members, &neighbors, &local_of);
    let mut eig = solve_component(&graph, Width::Spectral3d)
        .0
        .expect("6 <= DENSE_EIG_LIMIT solves");
    pin_signs(&mut eig);
    let mut coords = vec![0.0_f64; 6 * 3];
    scatter(&mut coords, &members, &eig, Width::Spectral3d);
    for d in 0..3 {
        assert_column_up_to_sign(
            &format!("axis {d}"),
            &column_from(&coords, 3, d),
            &column(&PATH6_SPECTRAL_BLOCK, d),
            SOLVER_TOLERANCE,
        );
    }
}

/// The same path through [`run_3d`], so the packing and the `_rescale_positions` multiply are
/// in the assertion too — a 3D row that stopped one step short of the reference's own
/// pipeline would still be a plane or an uncentred blob.
#[test]
fn six_node_path_three_d_is_the_reference_end_to_end() {
    let (geometry, reports) = run_3d(&path6(), LAYOUT_SEED).expect("one component, one solve");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(geometry.dim(), Dim::D3, "a 3D row must carry a z column");
    let (x, y, z) = columns(&geometry);
    let arms: [(&str, &[f32]); 3] = [("x", x), ("y", y), ("z", z)];
    for (d, (label, arm)) in arms.iter().enumerate() {
        let got: Vec<f64> = arm.iter().map(|&v| f64::from(v)).collect();
        assert_column_up_to_sign(label, &got, &column(&PATH6_SPECTRAL, d), GEOMETRY_TOLERANCE);
    }
}

/// Pivot MDS in 3D over the same path: `dist @ V` for the three largest eigenpairs of the
/// `k x k` Gram matrix, which is a different arithmetic from the spectral block above and
/// therefore its own table rather than a scaling of the other one.
#[test]
fn six_node_path_pivot_mds_three_d_is_the_reference_end_to_end() {
    let (geometry, reports) =
        crate::layout::pivot_mds::run_3d(&path6(), LAYOUT_SEED).expect("one component");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(geometry.dim(), Dim::D3);
    let (x, y, z) = columns(&geometry);
    let arms: [(&str, &[f32]); 3] = [("x", x), ("y", y), ("z", z)];
    for (d, (label, arm)) in arms.iter().enumerate() {
        let got: Vec<f64> = arm.iter().map(|&v| f64::from(v)).collect();
        assert_column_up_to_sign(label, &got, &column(&PATH6_MDS, d), GEOMETRY_TOLERANCE);
    }
}

/// The claim the header makes about the sign, checked on real solver output rather than
/// asserted in prose: on the 6-path at least one eigenvector has its largest magnitude
/// attained at **two or more** indices, so `_fix_eigenvector_signs`'s `argmax` has nothing
/// to prefer there and the flip is whichever side's last bit comes out larger.
///
/// This is what makes the up-to-sign comparison above the honest one. If a future path
/// changed shape until every column had a unique peak, this test would fail and the
/// comparison could be made exact — which is the point of naming it.
#[test]
fn a_six_path_eigenvector_has_a_tied_largest_magnitude_entry() {
    let members: Vec<u32> = (0..6).collect();
    let neighbors = Neighbors::from_rows(&path_neighbors(6));
    let local_of = local_positions(std::slice::from_ref(&members), 6);
    let graph = ComponentGraph::build(&members, &neighbors, &local_of);
    let mut eig = solve_component(&graph, Width::Spectral3d)
        .0
        .expect("6 <= DENSE_EIG_LIMIT solves");
    pin_signs(&mut eig);
    let ties: Vec<usize> = (0..eig.k)
        .map(|d| {
            let column: Vec<f64> = (0..6).map(|li| eig.column(d)[li]).collect();
            let (_, peak, tied) = peak_entry(&column);
            // The table stores exact equality; the claim is about magnitudes that agree to
            // solver precision, which is what makes `argmax`'s choice arbitrary.
            let near = column
                .iter()
                .filter(|v| (v.abs() - peak).abs() <= SOLVER_TOLERANCE * peak)
                .count();
            assert!(
                near >= tied.len(),
                "the exact tie is a subset of the near tie"
            );
            near
        })
        .collect();
    assert!(
        ties.iter().any(|&n| n >= 2),
        "no column of the 6-path has a tied peak ({ties:?}), \
so the up-to-sign comparison above could be exact"
    );
}

/// `_spectral_layout_3d:257-258`: below four nodes there is nothing to solve, and the
/// reference draws `_random_layout`. Asserted as a differential and not as a table, because
/// `layout::random`'s own tests already pin that draw's bits — what is new here is that the
/// spectral arm *is* that draw.
#[test]
fn below_four_nodes_the_three_d_arm_is_the_random_layout() {
    for n in [1usize, 2, 3] {
        let t = topology(n, &path_pairs(n));
        let (ours, reports) = run_3d(&t, LAYOUT_SEED).expect("never refuses");
        assert!(reports.is_empty(), "n={n}: nothing was attempted");
        let theirs = crate::layout::random::run_seeded(&t, LAYOUT_SEED).expect("never refuses");
        assert_eq!(ours, theirs, "n={n}: not _random_layout");
        assert!(
            columns(&ours).2.iter().any(|&v| v != 0.0),
            "n={n}: a z column of zeros is a plane, not the reference's draw"
        );
    }
}

/// The negative control for the test above: the fallback is the *seeded* draw, so a seed one
/// higher moves every coordinate. A `run_3d` that ignored its `seed` argument would print the
/// first test green and this one red.
#[test]
fn below_four_nodes_the_seed_moves_every_coordinate() {
    let t = topology(2, &path_pairs(2));
    let a = run_3d(&t, LAYOUT_SEED).expect("never refuses").0;
    let b = run_3d(&t, LAYOUT_SEED + 1).expect("never refuses").0;
    assert_ne!(a, b, "the n < 4 branch must read the seed it was handed");
}

/// Column `d` out of a row-major `n x dims` buffer.
fn column_from(buffer: &[f64], dims: usize, d: usize) -> Vec<f64> {
    (0..buffer.len() / dims)
        .map(|i| buffer[i * dims + d])
        .collect()
}
