//! `layout.grid`: nodes on a square lattice in index order. `Point` nodes, `Line` edges,
//! O(n). Reference: `SciGraphs/core/scigraphs_core/mesh/layouts/basic.py`
//! (`_grid_layout`): `cols = ceil(sqrt(n))`, node `i` in column `i % cols`, row `i / cols`.
//!
//! Two conventions, arbitrary but stated, because a snapshot hash pins them for good:
//!
//! - **Spacing.** Neighbouring cells are `spacing` apart on both axes (default `1.0`), so
//!   cells are square and the lattice's aspect is `cols : rows`, which `ceil(sqrt(n))`
//!   keeps at 1 or just above. SciGraphs instead fits the grid to its `scale`, at
//!   `scale / cols` per cell; nothing here is normalised to a box.
//! - **Centring.** The full `cols × rows` lattice, `rows = ceil(n / cols)`, is centred on
//!   the origin: `x = (col − (cols − 1) / 2) · spacing`, `y = (row − (rows − 1) / 2) ·
//!   spacing`. SciGraphs starts at the origin. `y` grows with the row; which way is down
//!   is the renderer's to decide.
//!
//! Exact: `cols` and `rows` are integer (`isqrt`, no float square root), and every
//! coordinate is one `f32` product of a half-integer below 2^16 and the spacing —
//! exact at the default spacing, correctly rounded at any other, alike on every target.
//!
//! Ponytail: the aspect is a convention, not a computation. When `n` is not a multiple of
//! `cols` the last row is ragged: it holds `n − (rows − 1) · cols` nodes from column 0,
//! and the centring is the lattice's, so the nodes' centroid sits off the origin (for
//! `n = 3`: two nodes over one, centroid at `(−1/6, −1/6)`). Direction: cosmetic, never
//! wrong — every node gets a distinct cell. Escape hatch: a later layout that centres
//! the last row, under its own id; this one's convention is pinned by its hash.

use super::Geometry;
use crate::index::Topology;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The grid stage.
#[derive(Debug, Clone, Copy)]
pub struct Grid;

/// The grid's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridParams {
    /// Distance between neighbouring cells, on both axes. Finite and above 0.
    pub spacing: f32,
}

impl Default for GridParams {
    /// Unit cells.
    fn default() -> Self {
        Self { spacing: 1.0 }
    }
}

impl Stage for Grid {
    type Params = GridParams;
    const ID: &'static str = "layout.grid";

    fn run(topology: &Topology, params: &GridParams) -> Result<Geometry, StageError> {
        let (x, y) = positions(topology.node_count(), params.spacing)?;
        Ok(Geometry {
            nodes: NodeGeometry::Point { x, y },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}

/// `(cols, rows)` for `n` nodes: `cols = ceil(sqrt(n))`, `rows = ceil(n / cols)`.
pub const fn dimensions(n: u32) -> (u32, u32) {
    if n == 0 {
        return (0, 0);
    }
    let root = n.isqrt();
    let cols = if root * root == n { root } else { root + 1 };
    (cols, n.div_ceil(cols))
}

/// Every node's centre, in index order.
fn positions(n: u32, spacing: f32) -> Result<(Vec<f32>, Vec<f32>), StageError> {
    if !(spacing.is_finite() && spacing > 0.0) {
        return Err(StageError::Param {
            name: "spacing",
            rule: "finite and above 0",
        });
    }
    let (cols, rows) = dimensions(n);
    let offset = |cell: u32, cells: u32| (cell as f32 - (cells - 1) as f32 / 2.0) * spacing;
    let x = (0..n).map(|i| offset(i % cols, cols)).collect();
    let y = (0..n).map(|i| offset(i / cols, rows)).collect();
    Ok((x, y))
}

#[cfg(test)]
mod tests {
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

    /// Worked by hand from the two conventions, not from the code.
    #[test]
    fn positions_match_the_hand_worked_grids() {
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
            let (x, y) = positions(n, 1.0).expect("unit spacing");
            let got: Vec<_> = x.into_iter().zip(y).collect();
            assert_eq!(got, expected, "n = {n}");
        }
        let (x, y) = positions(3, 2.5).expect("positive spacing");
        assert_eq!((x, y), (vec![-1.25, 1.25, -1.25], vec![-1.25, -1.25, 1.25]));
        assert_eq!(positions(0, 1.0).expect("empty"), (vec![], vec![]));
    }

    #[test]
    fn a_spacing_that_is_not_finite_and_positive_is_refused() {
        for spacing in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
            let err = positions(4, spacing).expect_err("refused");
            assert_eq!(
                err,
                StageError::Param {
                    name: "spacing",
                    rule: "finite and above 0"
                },
                "{spacing}"
            );
        }
        assert!(positions(4, f32::MIN_POSITIVE).is_ok());
    }

    #[test]
    fn the_largest_lattice_is_still_exact_half_integers() {
        let (cols, rows) = dimensions(u32::MAX);
        let offset = |cell: u32, cells: u32| cell as f32 - (cells - 1) as f32 / 2.0;
        assert_eq!(offset(0, cols), -32_767.5);
        assert_eq!(offset(cols - 1, cols), 32_767.5);
        assert_eq!(offset(rows - 1, rows), 32_767.5);
    }
}
