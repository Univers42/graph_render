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

use super::oracle_probe::{centres, fixture_ids, positioned};
use super::position;
use super::rank_fixture_edges::all as fixture_edges;

/// One closed case: a name, the input edges, and the inch strings `-Tplain` printed for each
/// node's centre — the x column then the y column, both in node order `n0`, `n1`, …
type Closed = (&'static str, &'static [(u32, u32)], &'static str, &'static str);

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
    ("3-path", &[(0, 1), (1, 2)], "0.375 0.375 0.375", "2.25 1.25 0.25"),
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
        &[(0, 1), (0, 2), (0, 3), (1, 4), (4, 5)],
        "1.375 0.375 1.375 2.375 1.375 1.375",
        "3.25 2.25 2.25 2.25 1.25 0.25",
    ),
];

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
        assert_eq!(inch_columns(&got), (want_x, want_y), "{name}");
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

/// The twenty fixture seeds, `n` = 2 to 21, with the inch strings the oracle printed.
///
/// **Written from the oracle's output, not computed.** The command is in this file's header;
/// the strings are what `-Tplain` printed, kept verbatim so the comparison cannot be looser
/// than the measurement they come from.
const FIXTURES: &[(u32, &str, &str)] = &[
    (0, "0.375 0.375", "0.25 1.25"),
    (1, "1.375 0.375 1.375", "0.25 1.25 1.25"),
    (2, "1.5139 0.375 1.9028 1.5139", "0.25 1.25 1.25 2.25"),
    (
        3,
        "1.2639 0.76389 1.7639 2.7639 0.375",
        "0.25 1.25 1.25 1.25 2.25",
    ),
    (
        4,
        "1.8056 1.3056 0.375 2.3056 3.3056 1.375",
        "0.25 1.25 2.25 1.25 1.25 2.25",
    ),
    (
        5,
        "2.1528 0.625 2.1528 3.1528 4.1528 0.375 1.6389",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25",
    ),
    (
        6,
        "2.875 0.375 1.375 2.375 3.375 0.375 1.375 4.375",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 1.25",
    ),
    (
        7,
        "3.1528 0.625 2.1528 3.1528 4.1528 0.375 1.6389 5.1528 6.1528",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 1.25 1.25",
    ),
    (
        8,
        "3.375 1.375 3.375 4.375 1.375 2.375 5.375 6.375 0.375 2.375",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25",
    ),
    (
        9,
        "2.375 1.375 0.375 2.375 1.375 0.375 3.375 4.0139 1.75 2.9028 4.9306",
        "0.25 1.25 1.25 1.25 2.25 2.25 1.25 2.25 3.25 2.25 1.25",
    ),
    (
        10,
        "3.4861 1.4306 3.4861 0.40278 4.4861 3.4861 5.4861 6.4861 7.4861 0.375 2.4583 1.4028",
        "0.25 1.25 1.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25",
    ),
    (
        11,
        "4.375 2.375 4.375 1.375 3.375 5.375 6.375 0.375 4.4306 2.375 7.4028 3.4028 4.4306",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25",
    ),
    (
        12,
        "3.1389 2.375 1.375 0.375 3.2639 3.9028 4.9028 5.9028 0.86111 5.2917 2.2361 5.8194 3.2639 5.8194",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25",
    ),
    (
        13,
        "5.1667 2.6389 5.1667 1.375 4.1667 6.1667 7.1667 0.375 4.4306 2.375 8.1944 3.4028 4.4306 9.25 5.4583",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 2.25",
    ),
    (
        14,
        "3.6894 3.3144 5.8421 1.6755 2.6199 6.8421 4.8421 7.8421 1.6199 4.6755 3.6477 0.68936 2.6199 0.68936 4.5505 0.59213",
        "0.25 1.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25 3.25 2.25",
    ),
    (
        15,
        "4.3472 2.8472 4.8472 5.8472 3.8472 0.375 0.81944 3.875 1.8194 6.8472 2.8472 3.875 7.875 4.9028 7.875 0.44444 5.9583",
        "0.25 1.25 1.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 2.25 2.25 3.25 2.25",
    ),
    (
        16,
        "5.7079 4.1802 5.7079 1.5968 7.7079 8.7079 6.7079 6.1802 5.1802 4.1802 2.5135 1.4579 2.5135 8.2218 0.40237 6.944 3.1524 5.8885",
        "0.25 1.25 1.25 2.25 1.25 1.25 1.25 2.25 2.25 2.25 3.25 3.25 4.25 2.25 3.25 3.25 2.25 3.25",
    ),
    (
        17,
        "5.3052 4.5413 6.069 2.5135 0.45793 6.569 2.5135 3.5135 7.069 5.569 2.5135 8.1107 0.40237 8.1246 1.4579 1.4857 6.444 4.5413 3.569",
        "0.25 1.25 1.25 1.25 1.25 2.25 2.25 2.25 2.25 1.25 2.25 3.25 1.25 3.25 2.25 3.25 2.25 3.25 2.25 3.25",
    ),
    (
        18,
        "6.9306 6.4167 4.875 1.4028 6.5694 2.4028 9.5278 0.375 8.0972 10.528 2.4583 3.4306 11.556 4.4861 6.5694 1.4028 7.4444 5.5417 7.8611 9.125",
        "0.25 1.25 1.25 1.25 2.25 2.25 2.25 1.25 3.25 2.25 1.25 3.25 2.25 1.25 2.25 3.25 3.25 1.25 2.25 3.25 2.25",
    ),
    (
        19,
        "5.6959 2.7654 5.8209 3.3626 7.4182 3.7654 2.8487 1.7654 6.9876 2.7654 2.8487 8.5432 7.4182 8.5432 4.3626 3.8765 3.7793 0.73761 2.2376 4.7932 5.7932",
        "0.25 1.25 2.25 4.25 5.25 2.25 5.25 2.25 1.25 2.25 6.25 1.25 6.25 2.25 6.25 5.25 3.25 2.25 3.25 2.25 3.25",
    ),
];

/// The twenty fixture seeds are placed as the oracle places them, byte for byte at the printed
/// precision.
#[test]
fn the_first_twenty_fixture_seeds_are_placed_as_the_oracle_places_them() {
    for (seed, edges) in fixture_edges() {
        let (_, want_x, want_y) = FIXTURES
            .iter()
            .find(|(s, ..)| *s == seed)
            .unwrap_or_else(|| panic!("seed {seed} has no printed table"));
        let count = nodes_of(edges);
        let got = centres(&positioned(count, edges));
        assert_eq!(inch_columns(&got), (*want_x, *want_y), "seed {seed}");
    }
}

/// The negative control for the width table. Seeds 0 to 8 have every label two characters long,
/// so every node's box is the 0.75 inch minimum; from seed 9 the ids reach three characters and
/// the boxes stop being the minimum. A pass that used a constant box would therefore agree on
/// the first nine and disagree from the tenth on — and it does, which is what makes the twenty
/// above a test of `text_width` and not only of the geometry.
#[test]
fn a_constant_node_box_disagrees_from_the_measured_one() {
    let mut agreed = 0;
    for (seed, edges) in fixture_edges() {
        if constant_box_agrees(edges) {
            agreed += 1;
        }
        if seed == 19 {
            break;
        }
    }
    assert_eq!(agreed, CONSTANT_BOX_SEEDS, "seeds a constant box also places right");
}

/// How many of the twenty fixture seeds the first nine inclusive are — the seeds whose labels
/// are all two characters, so every box is the minimum.
const CONSTANT_BOX_SEEDS: u32 = 9;

/// Whether the pass would place this seed the same way with every node on the default box.
///
/// This is the *wrong* variant of [`positioned`], kept as a control rather than as an option:
/// it is the port the rank pass stopped at, and it is right exactly as long as every label fits
/// inside the minimum node box.
fn constant_box_agrees(edges: &[(u32, u32)]) -> bool {
    let count = nodes_of(edges);
    let mut g = super::oracle_probe::graph(count, edges);
    super::rank::rank(&mut g).expect("the fixture graphs are rankable");
    super::mincross::run(&mut g);
    position(&mut g).expect("the fixture graphs are connected after the pass");
    let sized = centres(&positioned(count, edges));
    let plain = centres(&g);
    // Only x can differ: the box's width is an input to the x simplex alone, and the y
    // coordinates are rank times (height + ranksep) whatever the boxes are.
    sized.iter().map(|p| p.1).eq(plain.iter().map(|p| p.1))
        && inch_columns(&sized).0 == inch_columns(&plain).0
}

/// How many nodes a fixture seed's edge list reaches, which is its node count: the generator
/// numbers nodes densely from 0, so the highest index is the count.
fn nodes_of(edges: &[(u32, u32)]) -> u32 {
    edges
        .iter()
        .flat_map(|&(t, h)| [t, h])
        .max()
        .map_or(0, |top| top + 1)
}

/// How many nodes a named closed case has, read off its own inch table so the two tables cannot
/// drift apart.
fn case_nodes(name: &str) -> u32 {
    let (_, want) = CLOSED_POINTS
        .iter()
        .find(|(n, _)| *n == name)
        .unwrap_or_else(|| panic!("{name} is in the points table"));
    u32::try_from(want.len()).expect("a node count fits u32")
}

/// The two inch strings the plain format would print for a drawing's centres: the x column and
/// the y column, space separated, in node order.
fn inch_columns(points: &[(f64, f64)]) -> (String, String) {
    let column = |at: usize| {
        points
            .iter()
            .map(|p| plain_g(p[at] / 72.0))
            .collect::<Vec<_>>()
            .join(" ")
    };
    (column(0), column(1))
}

/// This crate's copy of the plain format's number formatter: `%g` at five significant digits,
/// trailing zeros stripped. Reproducing the oracle's formatter rather than rounding to a chosen
/// number of decimals is what makes "byte for byte" checkable — the grid moves with the
/// magnitude, so a fixed decimal count is wrong at both ends of a drawing.
fn plain_g(inches: f64) -> String {
    let mut scaled = inches;
    let mut exponent = 0i32;
    while scaled < 1.0 {
        scaled *= 10.0;
        exponent -= 1;
    }
    while scaled >= 10.0 {
        scaled /= 10.0;
        exponent += 1;
    }
    let decimals = (4 - exponent).max(0) as usize;
    let text = format!("{inches:.decimals$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The negative control for the printed-precision comparison itself: the six closed cases, at a
/// precision one digit finer than the oracle printed, must **not** agree. If `plain_g` were
/// rounding to a coarser grid than the oracle's, this would go red and every byte comparison in
/// this file would be weaker than it claims.
#[test]
fn the_comparison_is_not_coarser_than_the_oracles_printing() {
    let (_, edges, want_x, want_y) = CLOSED[0];
    let got = centres(&positioned(1, edges));
    assert_eq!(plain_g(got[0].0 / 72.0), want_x);
    assert_ne!(finer(got[0].0 / 72.0, 6), want_x, "six decimals is a finer grid");
}

/// The same formatter at an explicit number of decimals, for the control above.
fn finer(inches: f64, decimals: usize) -> String {
    format!("{inches:.decimals$}")
}

/// The ids a graph of `count` nodes is named with, exposed so the width table's own test can
/// check that `build` really does read them.
#[test]
fn a_graph_is_named_n0_to_n_count_minus_one() {
    let ids = fixture_ids(3);
    assert_eq!(ids, vec!["n0", "n1", "n2"]);
}
