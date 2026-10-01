//! The analytically determined cases, re-derived from `lib/pack/pack.c` rather than copied
//! out of the oracle, and the closed form pinned against the reference's own two sums.
//!
//! Every expectation here is the **closed answer** in the frame `-Tplain` prints: the
//! drawing translated so its lower-left corner is the origin, so node `n0` of a one-node
//! graph is at `27, 18` points = `0.375, 0.25` inch. That is the whole translation the
//! reference applies (`osageinit.c:198-220`), it is exact, and the oracle's own lines for
//! each case are in `harness/oracle-graphviz.py`'s `CLOSED` block as the independent check
//! that the closed answer and Graphviz agree — the oracle cannot be the oracle.

use super::sizes::{Boxes, NodeBox};
use super::{CELL_H, CELL_W, Grid, NODE_H, NODE_W, run, run_sized};
use crate::layout::coords::probe::{assert_close, graph, points};
use crate::stage::StageError;

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

// ---------------------------------------------------------------------------
// The explicit-size path: `run_sized` over the fixture size table.
//
// These are the cases that make the differential a real comparison. Below eleven nodes
// every box tied, so `arrayRects`'s `qsort` could not move the geometry and the closed
// cases above were all this engine had. `run_sized` is the reference's general
// `arrayRects` over per-node boxes, and the fixture table gives every node a distinct
// `width + height`, so the sort is a total order and the drawing is a different one from
// the uniform grid at every size. `docs/measurements/p13-gv1-osage.md` holds the sweep.
// ---------------------------------------------------------------------------

/// A uniform table of `count` boxes of one size, the shape the registry's own `run` uses.
fn uniform(count: u32, box_: NodeBox) -> Boxes {
    Boxes::new(vec![box_; count as usize])
}

/// The fixture table's own first box, and the default `nodesize` box it is measured against.
const FIRST: NodeBox = NodeBox { w: 54.0, h: 36.0 };

/// A table that is not one box per node is refused rather than read as this graph's answer:
/// `run_sized` is the entry point a caller reaches for, and the failure mode of ignoring the
/// length is a drawing of the wrong graph's boxes with no error anywhere.
#[test]
fn a_table_of_the_wrong_length_is_refused() {
    let topology = graph(4, &[(0, 1)]);
    let short = Boxes::new(vec![FIRST; 3]);
    assert_eq!(
        run_sized(&topology, &short).unwrap_err(),
        StageError::Param {
            name: "boxes",
            rule: "one box per node"
        },
        "a 3-box table was read as a 4-node graph's answer"
    );
    assert!(run_sized(&topology, &Boxes::new(vec![FIRST; 4])).is_ok());
}

/// One node, one box: the cell is the drawing, so the node is its own box's centre — the
/// sized answer to the one-node case above, at the table's first size.
#[test]
fn one_sized_node_sits_at_half_its_own_box() {
    let boxes = Boxes::new(vec![FIRST]);
    let got = points(&run_sized(&graph(1, &[]), &boxes).unwrap());
    assert_close(&got, &[(27.0, 18.0)], 1e-9);
}

/// **The uniform table must give the registry's own answer.** `run` is the closed form in
/// `grid.rs`; `run_sized` is `arrayRects` itself. They are two spellings of one reference
/// routine and this is the test that says so, over every node count the gate's model
/// reaches — without it, the sized path could drift from the path the ledger row runs and
/// the closed cases above would keep passing on a stale answer.
#[test]
fn a_uniform_table_gives_the_registereds_own_answer() {
    for count in 1..=600_u32 {
        let topology = graph(count, &[(0, 1)]);
        let boxes = uniform(count, FIRST);
        let got = points(&run_sized(&topology, &boxes).unwrap());
        let want = points(&run(&topology).unwrap());
        assert_eq!(got.len(), want.len());
        for (index, (g, w)) in got.iter().zip(&want).enumerate() {
            assert!(
                (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9,
                "n={count} i={index}: sized {g:?} vs registered {w:?}"
            );
        }
    }
}

/// **The fixture table's own two invariants**, and both are load-bearing rather than
/// tidy. `width + height` is what `acmpf` sorts on (`pack.c:569-579`), so a tie would put
/// the order back on glibc's unstable `qsort` — the second named cause of the disagreement
/// this job closed — and a table with no tie is what makes the port's `sort_by` exact.
/// And every box must be a whole number of `1/2048` inch, because Graphviz reads `width`
/// as inches and converts back to points; a size that does not survive that round trip
/// would make the two arms size the same box differently.
#[test]
fn the_fixture_table_has_no_sort_tie_and_survives_the_inch_round_trip() {
    let mut seen: Vec<f64> = Vec::new();
    for index in 0..600_u32 {
        let NodeBox { w, h } = Boxes::box_at(index);
        let sum = w + h;
        assert!(
            !seen.iter().any(|other| (other - sum).abs() < 1e-12),
            "node {index}: width+height {sum} ties an earlier node"
        );
        seen.push(sum);
        for (label, point) in [("width", w), ("height", h)] {
            let inches = point / 72.0;
            assert_eq!(
                inches * 72.0,
                point,
                "node {index}: {label} {point} does not survive the inch round trip"
            );
            assert_eq!(
                (inches * 2048.0).fract(),
                0.0,
                "node {index}: {label} {point} is not a whole number of 1/2048 inch"
            );
        }
    }
}

/// **The table is a permutation of the whole range, not a prefix of it**, so node 100 in a
/// six-hundred-node graph is no bigger than node 100 in a two-hundred-node one and a seed's
/// answer does not depend on how many nodes its graph happens to have.
#[test]
fn the_fixture_table_is_the_same_table_at_every_node_count() {
    for count in [1_u32, 2, 11, 200, 601] {
        let boxes = Boxes::table(count);
        let want: Vec<NodeBox> = (0..count).map(Boxes::box_at).collect();
        assert_eq!(boxes.get(), want, "count {count}");
    }
}

/// **The sort is what moves the nodes.** The table's `width + height` rises strictly with
/// the node index, so `acmpf`'s descending sort is exactly the reverse of the declaration
/// order: the *last* node takes the first cell. This is the case the uniform grid cannot
/// express — with every box tied, declaration order and sort order coincide and the sort is
/// untested. Eleven nodes is where the disagreement started, so it is the size this pins.
#[test]
fn the_sized_sort_puts_the_last_node_in_the_first_cell() {
    let count = 11_u32;
    let boxes = Boxes::table(count);
    let got = points(&run_sized(&graph(count, &[(0, 1)]), &boxes).unwrap());
    // nc = ceil(sqrt(11)) = 4 columns, nr = 3 rows, and the descending sort puts node 10 in
    // the first cell, so cell (r, c) holds node 10 - (r * 4 + c). Every width in column 0 is
    // the widest of its row, so the column edges are 0, 58.2109375, 116.38671875,
    // 174.52734375, 232.7734375; the row edges, counted up from the bottom, are 0, 40, 80,
    // 120.28125. Each node then sits at its own half-box inside its cell — which is why the
    // x values inside a column differ in the fourth decimal and the y values differ only
    // where a box is shorter than its row.
    //
    // The x values are written to seven digits, not to their exact `f64` expansion: the
    // comparison is on `f32` (the layout emits `f32`), every one of these is exact in `f32`,
    // and the longer spelling trips clippy's `excessive_precision` for no gain.
    let want = [
        (143.0, 18.0),
        (85.017_58, 18.0),
        (27.035_156, 18.0),
        (202.052_73, 58.0),
        (143.070_31, 58.0),
        (85.087_89, 58.0),
        (27.105_469, 58.0),
        (202.123_05, 98.0),
        (143.0, 98.140_625),
        (85.017_58, 98.140_625),
        (27.035_156, 98.140_625),
    ];
    assert_close(&got, &want, 1e-6);
}

/// The last node really is the one that moved, stated on its own so the assertion above
/// cannot pass by both arms drawing the same wrong order: node `n10` is the widest and the
/// tallest box in the table, so it sorts first and takes the top-left cell, while node `n0`
/// — the one the uniform grid put there — ends up in the last cell of the last row.
#[test]
fn the_widest_node_takes_the_first_cell_and_the_first_node_the_last() {
    let count = 11_u32;
    let boxes = Boxes::table(count);
    let got = points(&run_sized(&graph(count, &[(0, 1)]), &boxes).unwrap());
    let (node_ten_x, node_ten_y) = got[count as usize - 1];
    let (node_zero_x, node_zero_y) = got[0];
    assert!(
        node_ten_x < node_zero_x && node_ten_y > node_zero_y,
        "n10 at {node_ten_x},{node_ten_y} is not left of and above n0 at {node_zero_x},{node_zero_y}"
    );
    let last = boxes.get()[10];
    let first = boxes.get()[0];
    assert!(
        last.w + last.h > first.w + first.h,
        "n10 is not the largest box in the table"
    );
}

/// **The negative control for the whole job: the size table is load-bearing.** Changing one
/// node's box must move something. If `run_sized` ignored its second argument, or read only
/// the first box, this would still pass and the differential would be comparing the uniform
/// grid against a differently-sized Graphviz drawing while claiming to compare the sized
/// one. The sizes differ in `width + height`, so the sort order itself moves.
#[test]
fn changing_one_nodes_box_moves_the_drawing() {
    let count = 11_u32;
    let topology = graph(count, &[(0, 1)]);
    let base = Boxes::table(count);
    let got = points(&run_sized(&topology, &base).unwrap());

    let mut widened: Vec<NodeBox> = base.get().to_vec();
    let NodeBox { w, h } = widened[5];
    widened[5] = NodeBox { w: w + 400.0, h };
    let moved = points(&run_sized(&topology, &Boxes::new(widened)).unwrap());

    let differs = got
        .iter()
        .zip(&moved)
        .enumerate()
        .any(|(i, (a, b))| i != 5 && (a.0 - b.0).abs() > 1e-6 && (a.1 - b.1).abs() > 1e-6);
    assert!(
        differs,
        "one node's box changed and only its own cell moved"
    );
}
