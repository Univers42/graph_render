//! The `layout.treemap.patchwork` tests: the five closed cases, then the invariants.
//!
//! **The closed answers are the truth these cases carry.** A squarified treemap of `n`
//! equal-area nodes tiles a square of side `L = sqrt(1000 * n)` points — the reference
//! scales every node's default area of 1 up by 1000 so that 1 is a reasonable drawing size,
//! and the root square is `sqrt(total + 0.1)` with the fill rectangle shrunken by the margin
//! term that lands it back on `sqrt(1000 * n)`. Which tile each node gets is then a
//! statement about the *order* the rows are closed in, not about the arithmetic, so these
//! five restate that order as exact multiples of `L` and pin it node by node. They are not a
//! copy of the kernel: they are the closed form, and `docs/measurements/p13-gv1-patchwork.md`
//! records Graphviz's own printed answer for the same five graphs.

mod recursive;

use super::squarify;
use super::{ID, run};
use crate::layout::coords::probe::{assert_close, graph, points};

/// The field side for `n` equal-area nodes: `sqrt(1000 * n)` points.
///
/// Narrowed to `f32` because that is what the geometry carries: the kernel computes in
/// `f64` and `point_geometry` casts once on the way out, so the closed answers are compared
/// at the precision a reader of the geometry actually gets.
fn field(n: u32) -> f32 {
    (1000.0 * f64::from(n)).sqrt() as f32
}

#[test]
fn the_empty_graph_lays_out_to_nothing() {
    assert_eq!(points(&run(&graph(0, &[])).expect("runs")), vec![]);
}

#[test]
fn one_node_is_the_whole_field() {
    assert_eq!(
        points(&run(&graph(1, &[])).expect("runs")),
        vec![(0.0, 0.0)]
    );
}

/// Two equal squares side by side: the row closes after one, and the fill rectangle is as
/// wide as it is tall, so the row goes along `x`.
#[test]
fn two_nodes_split_the_field_in_half_along_x() {
    let l = field(2);
    assert_close(
        &points(&run(&graph(2, &[(0, 1)])).expect("runs")),
        &[(-l / 4.0, 0.0), (l / 4.0, 0.0)],
        1e-3,
    );
}

/// The first case where a row closes *and* the leftover rectangle turns over: two squares
/// along the top, then one spanning the full width underneath.
#[test]
fn three_nodes_close_a_row_of_two_then_fill_below() {
    let l = field(3);
    assert_close(
        &points(&run(&graph(3, &[(0, 1), (1, 2)])).expect("runs")),
        &[(-l / 4.0, l / 6.0), (l / 4.0, l / 6.0), (0.0, -l / 3.0)],
        1e-3,
    );
}

/// A 2x2 grid: two rows of two, and the second row's leftover is square again.
#[test]
fn four_nodes_tile_the_field_as_a_grid() {
    let l = field(4);
    let q = l / 4.0;
    assert_close(
        &points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)])).expect("runs")),
        &[(-q, q), (q, q), (-q, -q), (q, -q)],
        1e-3,
    );
}

/// The first case with three rows: a row of two along the top, then two rows of one each
/// stacked down the left, the last of them filling the width it is left.
#[test]
fn five_nodes_stack_three_rows() {
    let l = field(5);
    assert_close(
        &points(&run(&graph(5, &[(0, 1), (0, 2), (0, 3), (0, 4)])).expect("runs")),
        &[
            (-l / 4.0, 0.3 * l),
            (l / 4.0, 0.3 * l),
            (-l / 3.0, -0.2 * l),
            (0.0, -0.2 * l),
            (l / 3.0, -0.2 * l),
        ],
        1e-3,
    );
}

/// Every node lands inside the field, and the field is the square the closed answers
/// describe — the invariant that survives every `n`, where no closed answer is written out.
#[test]
fn every_node_lands_inside_the_field_for_every_size() {
    for n in [1_u32, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 552] {
        let half = field(n) / 2.0;
        let got = points(&run(&graph(n, &[])).expect("runs"));
        assert_eq!(got.len(), n as usize, "n={n}");
        for (i, &(x, y)) in got.iter().enumerate() {
            assert!(x.is_finite() && y.is_finite(), "n={n} node {i}: {got:?}");
            assert!(
                x.abs() <= half && y.abs() <= half,
                "n={n} node {i}: {got:?}"
            );
        }
    }
}

/// Area is conserved: the tiles exactly fill the field, so the sum of their areas is the
/// field's and no tile is left out. This is the invariant a coordinate-only check cannot
/// see, and it is the one that catches a row closed at the wrong index.
#[test]
fn the_tiles_conserv_the_fields_area_exactly() {
    for n in [1_u32, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 552] {
        let tiles = squarify::tile(n);
        let total: f64 = tiles.iter().map(|t| t.w * t.h).sum();
        let want = 1000.0 * f64::from(n);
        assert!(
            (total - want).abs() <= 1e-6 * want,
            "n={n}: {total} against {want}"
        );
    }
}

/// Every tile carries the same area — the flat graph gives every node the default area of 1
/// — so a tiling that is not uniform is a bug, not a shape.
#[test]
fn every_tile_carries_the_default_area() {
    for tile in squarify::tile(21) {
        assert!((tile.w * tile.h - 1000.0).abs() <= 1e-6, "{tile:?}");
    }
}

/// Two runs of the same graph are the same bytes: the kernel reads no clock and no
/// randomness, and its only order is the dense node index.
#[test]
fn the_layout_is_a_pure_function_of_the_topology() {
    let g = graph(
        9,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
        ],
    );
    assert_eq!(
        run(&g).expect("runs"),
        run(&g).expect("runs"),
        "two runs of one topology differ"
    );
}

/// The id is the one the registry and the hash gate file it under.
#[test]
fn the_id_names_the_engine() {
    assert_eq!(ID, "layout.treemap.patchwork");
}

/// The iterative fill and the reference's recursion produce the same tiles, node by node, at
/// every size. This is the test that licenses "iterative" as an implementation detail rather
/// than a second drawing.
#[test]
fn the_tiling_is_identical_to_the_recursive_form() {
    for n in [1_u32, 2, 3, 4, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 552] {
        let got = squarify::tile(n);
        let want = recursive::tile(n);
        assert_eq!(got.len(), want.len(), "n={n}");
        for (i, (g, w)) in got.iter().zip(&want).enumerate() {
            assert!(
                (g.x - w.x).abs() < 1e-9 && (g.y - w.y).abs() < 1e-9,
                "n={n} tile {i}: {g:?} against {w:?}"
            );
        }
    }
}

/// Where the recursive form's stack runs out, so `squarify`'s "iterative, and that is why"
/// note carries a measured number rather than an assertion.
///
/// **Ignored by default**, because the failing half of it *aborts the process* — a stack
/// overflow is a fatal runtime error, not a panic, so a normal test run would die rather than
/// report. One size per invocation for the same reason: a sweep in one process can only ever
/// report the first overflow.
///
/// ```sh
/// scripts/orch/gr -e PROBE_N=9340 cargo test -p graph-core --lib -- how_deep \
///   -- --ignored --nocapture
/// ```
///
/// Measured on this tree: `Ok(9_330)` at n=9 330, `STACK OVERFLOW` at n=9 340, on an 8 MiB
/// stack in a debug build. The iterative `tile` draws 1 000 000 nodes at flat depth (the
/// bench in `docs/measurements/p13-gv1-patchwork.md`).
#[test]
#[ignore]
fn how_deep_the_recursive_form_goes_before_the_stack_runs_out() {
    let n: u32 = std::env::var("PROBE_N")
        .expect("PROBE_N=<count> selects one size; an overflow aborts the process")
        .parse()
        .expect("PROBE_N is a number");
    let drawn = std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(move || recursive::tile(n).len())
        .expect("spawns")
        .join();
    println!("PROBE n={n} -> {drawn:?}");
    assert!(drawn.is_ok(), "the recursive form overflowed at n={n}");
}
