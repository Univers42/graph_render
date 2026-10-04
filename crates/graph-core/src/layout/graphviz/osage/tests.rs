//! The analytically determined cases, re-derived from `lib/pack/pack.c` rather than copied
//! out of the oracle, and the closed form pinned against the reference's own two sums.
//!
//! Every expectation here is the **closed answer** in the frame `-Tplain` prints: the
//! drawing translated so its lower-left corner is the origin, so node `n0` of a one-node
//! graph is at `27, 18` points = `0.375, 0.25` inch. That is the whole translation the
//! reference applies (`osageinit.c:198-220`), it is exact, and the oracle's own lines for
//! each case are in `harness/oracle-graphviz.py`'s `CLOSED` block as the independent check
//! that the closed answer and Graphviz agree — the oracle cannot be the oracle.

use super::{CELL_H, CELL_W, Grid, NODE_H, NODE_W, run};
use crate::layout::coords::probe::{assert_close, graph, points};

mod sized;

/// One cell of the grid: a `54 x 36` box with four points of margin, so columns are 58
/// apart and rows 40.
const STEP_X: f32 = CELL_W as f32;
const STEP_Y: f32 = CELL_H as f32;

/// Half a node box: the distance from a box's lower-left corner to its centre, which is
/// also the first node's own coordinate once the drawing sits on the origin.
const HALF_W: f32 = (NODE_W / 2.0) as f32;
const HALF_H: f32 = (NODE_H / 2.0) as f32;

#[test]
fn an_empty_graph_has_no_points() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
}

/// One node: `nr = nc = 1`, so the single cell is the drawing and the node is its centre.
/// Graphviz: `node n0 0.375 0.25`.
#[test]
fn one_node_sits_at_half_a_node_from_the_origin() {
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), [(HALF_W, HALF_H)]);
}

/// Two nodes: `nc = ceil(sqrt(2)) = 2` is the *column* count and `nr = ceil(2/2) = 1`, so
/// the pair is side by side — the row-major branch takes the column count from the square
/// root, not the row count. Graphviz: `n0 0.375 0.25`, `n1 1.1806 0.25`.
#[test]
fn two_nodes_sit_side_by_side_because_the_root_gives_the_column_count() {
    let want = [(HALF_W, HALF_H), (HALF_W + STEP_X, HALF_H)];
    assert_close(&points(&run(&graph(2, &[(0, 1)])).unwrap()), &want, 1e-3);
}

/// A 3-path and a 4-cycle have the same node count, so osage draws them **identically**:
/// it never reads an edge. `nc = 2`, `nr = 2`; nodes fill the top row left to right, then
/// the bottom row. Graphviz: `n0 0.375 0.80556`, `n1 1.1806 0.80556`, `n2 0.375 0.25`.
#[test]
fn the_edges_do_not_move_a_single_node() {
    let want = [
        (HALF_W, HALF_H + STEP_Y),
        (HALF_W + STEP_X, HALF_H + STEP_Y),
        (HALF_W, HALF_H),
    ];
    let path = points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap());
    let cycle = points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])).unwrap());
    assert_close(&path, &want, 1e-3);
    assert_close(&cycle[..3], &want, 1e-3);
    assert_close(&cycle[3..], &[(HALF_W + STEP_X, HALF_H)], 1e-3);
}

/// A 5-star: `nc = ceil(sqrt(5)) = 3`, `nr = ceil(5/3) = 2`, so the top row holds three and
/// the bottom row two, and the fifth node starts the second column of the bottom row —
/// the row-major fill does not pad. Graphviz: `n2 1.9861 0.80556`, `n3 0.375 0.25`.
#[test]
fn a_five_star_fills_row_by_row_without_padding() {
    let want = [
        (HALF_W, HALF_H + STEP_Y),
        (HALF_W + STEP_X, HALF_H + STEP_Y),
        (HALF_W + 2.0 * STEP_X, HALF_H + STEP_Y),
        (HALF_W, HALF_H),
        (HALF_W + STEP_X, HALF_H),
    ];
    let edges = [(0, 1), (0, 2), (0, 3), (0, 4)];
    assert_close(&points(&run(&graph(5, &edges)).unwrap()), &want, 1e-3);
}

/// A 6-branch is the first case whose grid is three wide and two high, so the bottom row
/// is full and the top row's three nodes are a whole cell-step above it.
#[test]
fn a_six_branch_grid_is_three_wide_and_two_high() {
    let edges = [(0, 1), (0, 2), (0, 3), (2, 4), (4, 5)];
    let want = [
        (HALF_W, HALF_H + STEP_Y),
        (HALF_W + STEP_X, HALF_H + STEP_Y),
        (HALF_W + 2.0 * STEP_X, HALF_H + STEP_Y),
        (HALF_W, HALF_H),
        (HALF_W + STEP_X, HALF_H),
        (HALF_W + 2.0 * STEP_X, HALF_H),
    ];
    assert_close(&points(&run(&graph(6, &edges)).unwrap()), &want, 1e-3);
}

/// Every coordinate is a whole number of points, which is what makes the comparison
/// against `-Tplain` a byte comparison rather than a tolerance: the port never rounds,
/// because the reference's `round` has nothing to round.
#[test]
fn every_coordinate_is_a_whole_number_of_points() {
    let count = 601_u32;
    for (x, y) in points(&run(&graph(count, &[(0, 1)])).unwrap()) {
        assert_eq!(x, x.trunc(), "{x} at a whole cell boundary");
        assert_eq!(y, y.trunc(), "{y} at a whole cell boundary");
    }
}

/// **The boundary, pinned on our side.** Eleven nodes is where this port stops agreeing
/// with the oracle, and the reason is measured rather than guessed: `n10`'s label is three
/// characters, Graphviz sizes its box at 57.942 points instead of 54, `arrayRects` takes
/// each column's cell width from the widest box in it, and glibc's unstable `qsort` then
/// puts the wide boxes first. So the reference draws `n0` at (89.0, 18.0) — last row,
/// second column, behind `n10` — where the equal-box grid this port implements puts it at
/// (27.0, 98.0), top row, first column. The grid is 4 x 3, not 3 x 4: `nc` comes from the
/// square root and `nr` from the remainder.
///
/// The expectation here is **ours**, stated so the boundary is a fact in the tree rather
/// than a discovery at the next gate: the oracle's own answer for this size is measured in
/// `docs/measurements/p13-gv1-osage.md`, and the gap is 176 points, not a rounding.
#[test]
fn eleven_nodes_is_where_the_oracle_stops_agreeing() {
    let want = [
        (HALF_W, HALF_H + 2.0 * STEP_Y),
        (HALF_W + STEP_X, HALF_H + 2.0 * STEP_Y),
        (HALF_W + 2.0 * STEP_X, HALF_H + 2.0 * STEP_Y),
        (HALF_W + 3.0 * STEP_X, HALF_H + 2.0 * STEP_Y),
        (HALF_W, HALF_H + STEP_Y),
        (HALF_W + STEP_X, HALF_H + STEP_Y),
        (HALF_W + 2.0 * STEP_X, HALF_H + STEP_Y),
        (HALF_W + 3.0 * STEP_X, HALF_H + STEP_Y),
        (HALF_W, HALF_H),
        (HALF_W + STEP_X, HALF_H),
        (HALF_W + 2.0 * STEP_X, HALF_H),
    ];
    let edges = [(0, 1), (0, 2), (0, 3), (1, 4), (4, 5)];
    assert_close(&points(&run(&graph(11, &edges)).unwrap()), &want, 1e-3);
}

/// **The closed form is the reference's two sums.** `Grid::rect` collapses
/// `arrayRects`'s width prefix sum and height suffix sum into two multiplies; this builds
/// those sums the way `pack.c:672-684` does and checks the collapse over 600 node counts,
/// which is every count the gate's own model reaches (`gate_node_count` is 2..601).
#[test]
fn the_closed_form_is_the_references_own_prefix_sums() {
    for count in 1..=600_u32 {
        let grid = Grid::of(count);
        let (widths, heights) = sums(count);
        for index in 0..count {
            let (column, row) = (index % grid.cols, index / grid.cols);
            let column = column as usize;
            let row = row as usize;
            let want_x = (widths[column] + widths[column + 1] - NODE_W) / 2.0;
            let want_y = (heights[row] + heights[row + 1] - NODE_H) / 2.0;
            let got = grid.rect(index);
            assert!(
                (got.0 - want_x).abs() < 1e-9 && (got.1 - want_y).abs() < 1e-9,
                "n={count} i={index}: {got:?} vs ({want_x}, {want_y})"
            );
        }
    }
}

/// `arrayRects`' own position arrays, rebuilt: `widths` is the column prefix sum and
/// `heights` the row suffix sum counted up from the bottom row, each one longer than its
/// count so index `c + 1` / `r + 1` is the far edge of the cell.
fn sums(count: u32) -> (Vec<f64>, Vec<f64>) {
    let grid = Grid::of(count);
    let cols = grid.cols as usize;
    let rows = grid.rows as usize;
    let mut widths = vec![0.0; cols + 1];
    let mut heights = vec![0.0; rows + 1];
    for cell in widths.iter_mut().take(cols) {
        *cell = CELL_W;
    }
    for cell in heights.iter_mut().take(rows) {
        *cell = CELL_H;
    }
    let mut run_width = 0.0;
    for cell in widths.iter_mut() {
        let value = *cell;
        *cell = run_width;
        run_width += value;
    }
    let mut run_height = 0.0;
    for row in (1..=rows).rev() {
        let value = heights[row - 1];
        heights[row] = run_height;
        run_height += value;
    }
    heights[0] = run_height;
    (widths, heights)
}
