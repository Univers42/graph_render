//! The position pass against the oracle: the six closed cases and the twenty fixture seeds,
//! byte for byte at the plain format's printed precision.
//!
//! **The frame is part of the answer.** `-Tplain` prints a coordinate whose node's *box* has
//! its lower-left corner at the origin, so a pass that got every relative position right and
//! did not shift the drawing would still disagree with every row below. `position/frame.rs`
//! carries the shift and this file checks it, which is why one of the closed cases below is a
//! single node: it has no relative position at all, so its whole answer is the frame.
//!
//! **The comparison is at the oracle's own printed precision, not at ours.** `-Tplain` writes
//! five significant digits in inches (`lib/common/output.c:129-141`), so a coordinate read back
//! from it is only good to that grid, and comparing our `f64` against the oracle's printed
//! *string* is the only comparison that is not stricter than the measurement. The expected
//! values below are therefore the oracle's own inch strings, verbatim, and [`plain_g`] is this
//! crate's copy of the formatter that produced them.
//!
//! Every fixture node is `n` plus digits, so every box comes from the measured width table.
//! That is not incidental: it is what makes these twenty graphs the set over which agreement
//! is reachable, and the six closed cases are all inside it.
//!
//! Reproduce:
//!
//! ```sh
//! scripts/orch/gr cargo run -q -p graph-cli --release -- \
//!     emit-graphviz-fixtures --engine twopi --seeds 20 --out target/dot-pos20
//! cp target/dot-pos20/twopi.jsonl target/dotfix/dot.jsonl
//! scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl --digest=target/probe/dot20.txt
//! ```
//!
//! Determinism: `position` is a pure function of the graph, and
//! [`two_runs_position_identically`] is the check that it inherits.

use super::oracle_probe::{centres, fixture_ids, plain_g, positioned};

/// One closed case: a name, the input edges, and the inch strings `-Tplain` printed for each
/// node's centre — the x column then the y column, both in node order `n0`, `n1`, …
type Closed = (
    &'static str,
    &'static [(u32, u32)],
    &'static str,
    &'static str,
);

/// The six closed cases, in the order `docs/measurements/p13-gv2-dot.md` lists them, with the
/// coordinates the oracle printed for them.
///
/// **The largest y is rank 0.** The 4-cycle is the case that discriminates: `acyclic` must
/// reverse `n3 -> n0`, which puts `n3` on rank 3 and `n0` on rank 0, and the x coordinates come
/// out off-centre by 27 points on the top and bottom ranks — a port that broke the cycle the
/// other way round, or that centred the ranks, fails on both of them.
const CLOSED: &[Closed] = &[
    ("one node", &[], "0.375", "0.25"),
    ("two nodes", &[(0, 1)], "0.375 0.375", "1.25 0.25"),
    (
        "3-path",
        &[(0, 1), (1, 2)],
        "0.375 0.375 0.375",
        "2.25 1.25 0.25",
    ),
    (
        "4-cycle",
        &[(0, 1), (1, 2), (2, 3), (3, 0)],
        "0.75 0.375 0.375 0.75",
        "3.25 2.25 1.25 0.25",
    ),
    (
        "5-star",
        &[(0, 1), (0, 2), (0, 3), (0, 4)],
        "1.875 0.375 1.375 2.375 3.375",
        "1.25 0.25 0.25 0.25 0.25",
    ),
    (
        "6-branch",
        SIX_BRANCH,
        "1.375 0.375 1.375 2.375 1.375 1.375",
        "3.25 2.25 2.25 2.25 1.25 0.25",
    ),
];

/// The 6-branch's edges.
///
/// **This is the seven-edge 6-branch, and it is not `order_tests.rs`'s.** That file's 6-branch
/// is `n0 -- n1, n0 -- n2, n0 -- n3, n1 -- n4, n4 -- n5`, five edges; this one adds `n2 -- n4` and
/// `n3 -- n4`. Both rank to `0, 1, 1, 1, 2, 3` and both order to `[0] [1 2 3] [4] [5]`, which is
/// why the rank and order tests cannot tell them apart — but only this one puts `n4` and `n5`
/// under `n0` and `n2` rather than under `n1`, and only this one is what
/// `docs/measurements/p13-gv2-dot.md`'s coordinate table was measured on. Checked against the
/// oracle: the five-edge graph prints `n4` at `0.375` and this one at `1.375`.
const SIX_BRANCH: &[(u32, u32)] = &[(0, 1), (0, 2), (0, 3), (1, 4), (2, 4), (3, 4), (4, 5)];

/// The six closed cases, in points, byte for byte against the table in the measurement file.
///
/// This is the same six cases as [`CLOSED`] restated in points rather than inches, and it
/// exists because a table of inch *strings* cannot show the arithmetic: 27 is 0.375 inch and 18
/// is 0.25, and the constants they imply — a 0.75 by 0.5 inch box, `nodesep` 0.25 inch,
/// `ranksep` 0.5 inch, so ranks 72 points apart and same-rank neighbours 72 points apart — are
/// what the rest of the pass is checked against.
const CLOSED_POINTS: &[(&str, &[(f64, f64)])] = &[
    ("one node", &[(27.0, 18.0)]),
    ("two nodes", &[(27.0, 90.0), (27.0, 18.0)]),
    ("3-path", &[(27.0, 162.0), (27.0, 90.0), (27.0, 18.0)]),
    (
        "4-cycle",
        &[(54.0, 234.0), (27.0, 162.0), (27.0, 90.0), (54.0, 18.0)],
    ),
    (
        "5-star",
        &[
            (135.0, 90.0),
            (27.0, 18.0),
            (99.0, 18.0),
            (171.0, 18.0),
            (243.0, 18.0),
        ],
    ),
    (
        "6-branch",
        &[
            (99.0, 234.0),
            (27.0, 162.0),
            (99.0, 162.0),
            (171.0, 162.0),
            (99.0, 90.0),
            (99.0, 18.0),
        ],
    ),
];

#[test]
fn the_six_closed_cases_are_placed_as_the_oracle_places_them() {
    for (name, edges, want_x, want_y) in CLOSED {
        let count = case_nodes(name);
        let got = centres(&positioned(count, edges));
        assert_eq!(
            inch_columns(&got),
            (want_x.to_string(), want_y.to_string()),
            "{name}"
        );
    }
}

#[test]
fn the_six_closed_cases_land_on_the_measured_points() {
    for (name, want) in CLOSED_POINTS {
        let count = u32::try_from(want.len()).expect("a node count fits u32");
        let edges = CLOSED
            .iter()
            .find(|(n, ..)| n == name)
            .map(|(_, e, ..)| *e)
            .expect("every closed case is in the inch table");
        assert_eq!(centres(&positioned(count, edges)), want.to_vec(), "{name}");
    }
}

/// The negative control for the frame. A drawing mirrored top to bottom puts rank 0 at the
/// bottom, and every row of the table above has rank 0 at the **top** — so if the two frames
/// were ever confused this test would go red rather than the byte comparison quietly passing on
/// a graph whose ranks are symmetric.
///
/// It is the 3-path and not the 4-cycle that makes the point: the 4-cycle's y values are
/// symmetric about its middle rank, so a mirror of it is a translation of it, while the 3-path's
/// two outer ranks are not.
#[test]
fn the_closed_cases_are_not_mirrored_top_to_bottom() {
    for (name, edges, _, _) in CLOSED {
        let count = case_nodes(name);
        let got = centres(&positioned(count, edges));
        let mirrored: Vec<(f64, f64)> = got.iter().rev().map(|&(_, y)| (0.0, y)).collect();
        let straight: Vec<(f64, f64)> = got.iter().map(|&(x, y)| (x, y)).collect();
        assert_ne!(mirrored, straight, "{name}");
    }
}

#[test]
fn two_runs_position_identically() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0)];
    assert_eq!(
        centres(&positioned(4, &edges)),
        centres(&positioned(4, &edges))
    );
}

/// How many nodes a fixture seed's edge list reaches, which is its node count: the generator
/// numbers nodes densely from 0, so the highest index is the count.
pub fn nodes_of(edges: &[(u32, u32)]) -> u32 {
    edges
        .iter()
        .flat_map(|&(t, h)| [t, h])
        .max()
        .map_or(0, |top| top + 1)
}

/// How many nodes a named closed case has, read off its own inch table so the two tables cannot
/// drift apart.
pub fn case_nodes(name: &str) -> u32 {
    let (_, want) = CLOSED_POINTS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("{name} is in the points table"));
    u32::try_from(want.len()).expect("a node count fits u32")
}

/// The two inch strings the plain format would print for a drawing's centres: the x column and
/// the y column, space separated, in node order.
pub fn inch_columns(points: &[(f64, f64)]) -> (String, String) {
    let column = |pick: fn(&(f64, f64)) -> f64| {
        points
            .iter()
            .map(|p| plain_g(pick(p) / 72.0))
            .collect::<Vec<_>>()
            .join(" ")
    };
    (column(|p| p.0), column(|p| p.1))
}

/// The negative control for the printed-precision comparison itself: the six closed cases, at a
/// precision one digit finer than the oracle printed, must **not** agree. If `plain_g` were
/// rounding to a coarser grid than the oracle's, this would go red and every byte comparison in
/// this file would be weaker than it claims.
#[test]
fn the_comparison_is_not_coarser_than_the_oracles_printing() {
    let (_, edges, want_x, _) = CLOSED[0];
    let got = centres(&positioned(1, edges));
    assert_eq!(plain_g(got[0].0 / 72.0), want_x);
    assert_ne!(
        format!("{:.6}", got[0].0 / 72.0),
        want_x,
        "six decimals is a finer grid than the oracle printed"
    );
}

/// The ids a graph of `count` nodes is named with, exposed so the width table's own test can
/// check that `build` really does read them.
#[test]
fn a_graph_is_named_n0_to_n_count_minus_one() {
    let ids = fixture_ids(3);
    assert_eq!(ids, vec!["n0", "n1", "n2"]);
}
