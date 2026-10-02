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
//! SciGraphs' pair of them is [`Grid::run_scaled`] — the first cell at the origin at a
//! pitch of `scale / cols` — in `scaled.rs`, where the multiply-then-divide is `f64`
//! because a `f32` `spacing` cannot reach those bits. The registered [`Grid::run`] below
//! is neither that nor anything else changed: these two conventions are what its snapshot
//! hash is.
//!
//! Exact: `cols` and `rows` are integer (`isqrt`, no float square root), and every
//! registered coordinate is one `f32` product of a half-integer below 2^16 and the spacing —
//! exact at the default spacing, correctly rounded at any other, alike on every target.
//!
//! Phase 11: [`Grid::run_with`] hands the per-node gather to a runner, and **there is no
//! merge below it** — the grid is `f32` end to end and `coords::rescale` is not in its
//! path, so its coordinates are a pure function of the node's own index. The stage is
//! therefore the whole kernel: a width is a schedule of it, and the only thing a wrong
//! partition of these outputs could get wrong is a range boundary, which is what
//! `the_lattice_is_the_same_bytes_at_every_worker_count` and the per-width hash-gate arms
//! are for. `GM_MUTATE_GRID_SPACING` stays the stage's own control for a different
//! question — it moves a parameter, so it moves every arm alike and proves the stage is
//! hashed and compared at all, where the threaded arms need a control that only they can
//! fail.
//!
//! Ponytail: the aspect is a convention, not a computation. When `n` is not a multiple of
//! `cols` the last row is ragged: it holds `n − (rows − 1) · cols` nodes from column 0,
//! and the centring is the lattice's, so the nodes' centroid sits off the origin (for
//! `n = 3`: two nodes over one, centroid at `(−1/6, −1/6)`). Direction: cosmetic, never
//! wrong — every node gets a distinct cell. Escape hatch: a later layout that centres
//! the last row, under its own id; this one's convention is pinned by its hash.

use super::Geometry;
use crate::exec::{Runner, Serial, StepRange};
use crate::index::Topology;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use std::ops::Range;

mod scaled;

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

    /// The serial tier, which is the stage: [`Grid::run_with`] over [`Serial`] with one
    /// worker, and the arm every other runner must hash-equal.
    fn run(topology: &Topology, params: &GridParams) -> Result<Geometry, StageError> {
        Self::run_with(topology, params, &Serial, 1)
    }
}

impl Grid {
    /// The same lattice, its per-node gather handed to `runner` over `workers` workers.
    ///
    /// The point of the signature is what it does **not** change: `cols`, `rows`, the
    /// centring and the one `f32` product per coordinate are the same code the serial
    /// stage runs, and only the division of the gather differs. So `run_with(..., &Serial,
    /// 1)` is [`Stage::run`] by construction, and any other runner is a *schedule* of the
    /// same computation.
    ///
    /// **The grid has no merge, so there is nothing here for a control to corrupt** — the
    /// coordinates are a pure function of the node's own index, and `coords::rescale` is
    /// not in the grid's path at all (it is `f32` end to end, where `rescale` narrows
    /// from `f64`). That is why the grid's negative control is
    /// `GM_MUTATE_GRID_SPACING` — a *parameter*, which moves every arm, scalar and
    /// threaded alike, and so answers "is this stage hashed and compared at all?" rather
    /// than "did the threaded arm recompute the merge?". The answer to the second question
    /// for the grid is a range-boundary claim, and a width of 1, 2, 3, 4 and 7 is what it
    /// takes.
    pub fn run_with(
        topology: &Topology,
        params: &GridParams,
        runner: &impl Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        let mut pairs = Vec::new();
        runner.run(
            &cells(topology.node_count(), params.spacing)?,
            workers,
            &mut pairs,
        );
        let (x, y): (Vec<f32>, Vec<f32>) = pairs.into_iter().unzip();
        Ok(Geometry::planar(
            NodeGeometry::Point { x, y },
            EdgeGeometry::Line,
            Vec::new(),
        ))
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

/// The lattice kernel over `n` nodes at `spacing`, or the parameter error a bad spacing is.
///
/// The validation lives here, **once**: the serial stage and every threaded arm build the
/// kernel through this, so there is no second place a spacing rule could be stated.
fn cells(n: u32, spacing: f32) -> Result<Lattice, StageError> {
    if !(spacing.is_finite() && spacing > 0.0) {
        return Err(StageError::Param {
            name: "spacing",
            rule: "finite and above 0",
        });
    }
    let (cols, rows) = dimensions(n);
    Ok(Lattice {
        count: n,
        cols,
        rows,
        spacing,
    })
}

/// Every node's centre, in index order: node `i` in column `i % cols`, row `i / cols`.
///
/// A [`StepRange`] whose `Out` is the node's own `(x, y)`, so one worker writes both of a
/// node's coordinates and the two columns cannot come out misaligned however the range was
/// cut. `&self` and no `&mut`: a worker reads the cell count and the spacing and writes
/// only `out[i - range.start]`, so `0..n` divides any way at all (D10).
#[derive(Debug)]
struct Lattice {
    count: u32,
    cols: u32,
    rows: u32,
    spacing: f32,
}

impl Lattice {
    /// The centre of `cell` in a run of `cells` — the lattice's own centring, the one
    /// product the hot loop is.
    fn offset(&self, cell: u32, cells: u32) -> f32 {
        (cell as f32 - (cells - 1) as f32 / 2.0) * self.spacing
    }
}

impl StepRange for Lattice {
    type Out = (f32, f32);

    fn len(&self) -> u32 {
        self.count
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f32, f32)]) {
        for (i, slot) in range.zip(out) {
            *slot = (
                self.offset(i % self.cols, self.cols),
                self.offset(i / self.cols, self.rows),
            );
        }
    }
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
}
