//! The registered `layout.grid` kernel's own tests, split out of `grid.rs` by the house
//! file limit: the module is within a few lines of 300 and these tests are half of it.
//!
//! The sibling `layout/grid/scaled.rs` keeps its tests inline because it is a
//! single-purpose module and well under the cap; this one is the tree's convention
//! everywhere else.

use super::*;

#[test]
fn dimensions_are_ceil_sqrt_columns_and_ceil_rows() {
    let expected = [
        (0, (0, 0)),
        (1, (1, 1)),
        (2, (2, 1)),
        (3, (2, 2)),
        (4, (2, 2)),
        (5, (3, 2)),
        (9, (3, 3)),
        (10, (4, 3)),
        (50, (8, 7)),
        (601, (25, 25)),
        (u32::MAX, (65_536, 65_536)),
    ];
    for (n, dims) in expected {
        assert_eq!(dimensions(n), dims, "n = {n}");
    }
}

/// Worked by hand from the two conventions, not from the code, and read back out of the
/// kernel the threaded arms run — so the table and the tier cannot be two drawings.
#[test]
fn the_lattice_matches_the_hand_worked_grids() {
    let cases: [(u32, &[(f32, f32)]); 5] = [
        (1, &[(0.0, 0.0)]),
        (2, &[(-0.5, 0.0), (0.5, 0.0)]),
        (3, &[(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5)]),
        (4, &[(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)]),
        (
            5,
            &[
                (-1.0, -0.5),
                (0.0, -0.5),
                (1.0, -0.5),
                (-1.0, 0.5),
                (0.0, 0.5),
            ],
        ),
    ];
    for (n, expected) in cases {
        assert_eq!(centres(n, 1.0), expected, "n = {n}");
    }
    assert_eq!(
        centres(3, 2.5),
        vec![(-1.25, -1.25), (1.25, -1.25), (-1.25, 1.25)]
    );
    assert_eq!(centres(0, 1.0), vec![]);
}

/// Every node's centre, gathered by the kernel itself — the one the threaded arms run.
fn centres(n: u32, spacing: f32) -> Vec<(f32, f32)> {
    let lattice = cells(n, spacing).expect("a spacing the rule allows");
    let mut out = vec![(0.0, 0.0); n as usize];
    lattice.step_range(0..n, &mut out);
    out
}

/// The gather is a per-node function, so every width writes the same cell centres: this
/// is the grid's byte-identity claim at the unit level, and the hash gate's per-width
/// arms are the same statement over the gate's own models.
#[test]
fn the_lattice_is_the_same_bytes_at_every_worker_count() {
    for n in [0, 1, 2, 3, 5, 16, 17, 64, 1000] {
        let want = centres(n, 1.0);
        for workers in [1, 2, 3, 4, 7] {
            let topology = crate::layout::coords::probe::graph(n, &[]);
            let got = Grid::run_with(&topology, &GridParams { spacing: 1.0 }, &Serial, workers)
                .expect("unit spacing");
            let NodeGeometry::Point { x, y } = got.nodes else {
                panic!("point nodes");
            };
            let pairs: Vec<(f32, f32)> = x.into_iter().zip(y).collect();
            assert_eq!(pairs, want, "n = {n}, workers = {workers}");
        }
    }
}

#[test]
fn a_spacing_that_is_not_finite_and_positive_is_refused() {
    for spacing in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
        let err = cells(4, spacing).expect_err("refused");
        assert_eq!(
            err,
            StageError::Param {
                name: "spacing",
                rule: "finite and above 0"
            },
            "{spacing}"
        );
    }
    assert!(cells(4, f32::MIN_POSITIVE).is_ok());
}

#[test]
fn the_largest_lattice_is_still_exact_half_integers() {
    let (cols, rows) = dimensions(u32::MAX);
    let offset = |cell: u32, cells: u32| cell as f32 - (cells - 1) as f32 / 2.0;
    assert_eq!(offset(0, cols), -32_767.5);
    assert_eq!(offset(cols - 1, cols), 32_767.5);
    assert_eq!(offset(rows - 1, rows), 32_767.5);
}
