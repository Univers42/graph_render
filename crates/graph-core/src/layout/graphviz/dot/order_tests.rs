//! The order of the nodes within each rank, against the oracle: the six closed cases and
//! twenty fixture seeds, pinned row by row.
//!
//! **An order is only comparable when the ranks are.** Both tables below are read off the
//! oracle's printed coordinates — a node's rank from its y, its place within the rank from
//! its x — so the ranks here are the same ones `rank_tests.rs` pins, and a row of the oracle
//! that disagreed with the port's ranks would be a rank disagreement and not an order one.
//!
//! The six closed cases are the table under "The six closed cases" in
//! `docs/measurements/p13-gv2-dot.md`, sorted the way the probe sorts: by x within a rank.
//! Four of the six have one node per rank, so it is the 5-star and the 6-branch that carry
//! the order, and the 5-star's four leaves are the case that discriminates: they are
//! interchangeable to the rank pass and to the crossing count, and only the order pass puts
//! `n1` leftmost.
//!
//! ```sh
//! scripts/orch/gr cargo run -q -p graph-cli --release -- \\
//!     emit-graphviz-fixtures --engine twopi --seeds 20 --out target/dot-probe1000
//! cp target/dot-probe1000/twopi.jsonl target/dotfix/dot.jsonl
//! scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \\
//!     python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl --table=20
//! ```
//!
//! Determinism: the pass is a pure function of the graph, and the tables are read from the
//! oracle, which `docs/measurements/p13-gv2-dot.md` measures to be byte-identical over the
//! full 1000 seeds.

use super::oracle_probe::ordered;
use super::rank_fixture_edges::FIXTURE_EDGES;

/// One closed case: a name, the input edges, and the oracle's rows, rank 0 first.
type Closed = (&'static str, &'static [(u32, u32)], &'static [&'static [u32]]);

/// The six closed cases, in the order `docs/measurements/p13-gv2-dot.md` lists them, with
/// each rank's nodes in the order the oracle printed them in x.
const CLOSED: &[Closed] = &[
    ("one node", &[], &[&[0]]),
    ("two nodes", &[(0, 1)], &[&[0], &[1]]),
    ("3-path", &[(0, 1), (1, 2)], &[&[0], &[1], &[2]]),
    ("4-cycle", &[(0, 1), (1, 2), (2, 3), (3, 0)], &[&[0], &[1], &[2], &[3]]),
    ("5-star", &[(0, 1), (0, 2), (0, 3), (0, 4)], &[&[0], &[1, 2, 3, 4]]),
    (
        "6-branch",
        &[(0, 1), (0, 2), (0, 3), (1, 4), (4, 5)],
        &[&[0], &[1, 2, 3], &[4], &[5]],
    ),
];

#[test]
fn the_six_closed_cases_are_ordered_as_the_oracle_orders_them() {
    for (name, edges, want) in CLOSED {
        let count = u32::try_from(graph_nodes(want)).expect("a node count fits u32");
        assert_eq!(ordered(count, edges), rows_of(want), "{name}");
    }
}

/// The table's rows as the pass hands them back, so the table can stay a literal.
fn rows_of(rows: &[&[u32]]) -> Vec<Vec<u32>> {
    rows.iter().map(|row| row.to_vec()).collect()
}

/// How many nodes a case's rows hold, which is the node count the case is built from.
fn graph_nodes(rows: &[&[u32]]) -> usize {
    rows.iter().map(|row| row.len()).sum()
}

#[test]
fn the_six_closed_cases_are_not_their_own_reverse() {
    // The negative control for the table above. A rank has no direction of its own, so a port
    // that laid every rank out right to left would reproduce all six closed cases with their
    // ranks untouched, and only this catches it. It must fail on every case with a row of
    // more than one node, and pass on the ones that have none.
    for (name, edges, want) in CLOSED {
        let flipped: Vec<Vec<u32>> = want.iter().map(|row| row.iter().rev().copied().collect()).collect();
        let count = u32::try_from(graph_nodes(want)).expect("a node count fits u32");
        if want.iter().any(|row| row.len() > 1) {
            assert_ne!(ordered(count, edges), flipped, "{name} must not be mirrored");
        } else {
            assert_eq!(ordered(count, edges), flipped, "{name} has nothing to mirror");
        }
    }
}

/// Twenty fixture seeds, one row per rank, as `harness/oracle-dot-probe.py --table` read the
/// oracle's printed coordinates. `n` runs 2 to 21, and these are the shapes the closed cases
/// are not: a rank of seven, a rank of six, and graphs whose initial order the sweeps have
/// to change rather than accept.
const FIXTURES: &[&[&[u32]]] = &[
    &[&[1], &[0]],
    &[&[1, 2], &[0]],
    &[&[3], &[1, 2], &[0]],
    &[&[4], &[1, 2, 3], &[0]],
    &[&[2, 5], &[1, 3, 4], &[0]],
    &[&[5, 6], &[1, 2, 3, 4], &[0]],
    &[&[5, 6], &[1, 2, 3, 4, 7], &[0]],
    &[&[5, 6], &[1, 2, 3, 4, 7, 8], &[0]],
    &[&[8, 4, 9], &[1, 5, 2, 3, 6, 7], &[0]],
    &[&[8], &[5, 4, 9, 7], &[2, 1, 3, 6, 10], &[0]],
    &[&[9, 11, 10, 5], &[3, 1, 2, 4, 6, 7, 8], &[0]],
    &[&[12], &[7, 3, 9, 11, 8], &[1, 4, 2, 5, 6, 10], &[0]],
    &[&[13], &[12, 11], &[8, 10, 4, 9], &[3, 2, 1, 5, 6, 7], &[0]],
    &[&[12], &[7, 3, 9, 11, 8, 14], &[1, 4, 2, 5, 6, 10, 13], &[0]],
    &[&[13], &[11, 12, 14], &[15, 8, 4, 10, 9], &[3, 1, 6, 2, 5, 7], &[0]],
    &[&[15, 11], &[6, 8, 10, 7, 13, 16, 14], &[5, 1, 4, 2, 3, 9, 12], &[0]],
    &[
        &[12],
        &[14, 11, 10, 17, 15],
        &[3, 16, 9, 8, 7, 13],
        &[1, 2, 6, 4, 5],
    ],
    &[
        &[12, 14, 10, 18, 16],
        &[4, 15, 6, 7, 17, 9, 5, 13],
        &[3, 1, 2, 8, 11],
        &[0],
    ],
    &[
        &[7, 15, 10, 14, 18],
        &[3, 5, 11, 13, 17, 4, 8, 19],
        &[2, 1, 16, 6, 9, 12],
        &[0],
    ],
    &[
        &[10, 14, 12],
        &[6, 15, 4],
        &[3],
        &[18, 16, 20],
        &[17, 7, 9, 5, 19, 2, 13],
        &[1, 8, 11],
        &[0],
    ],
];

#[test]
fn the_first_twenty_fixture_seeds_are_ordered_as_the_oracle_orders_them() {
    assert_eq!(FIXTURES.len(), FIXTURE_EDGES.len());
    for ((seed, edges), rows) in FIXTURE_EDGES.iter().zip(FIXTURES) {
        let count = u32::try_from(rows.iter().map(|row| row.len()).sum::<usize>())
            .expect("a node count fits u32");
        assert_eq!(ordered(count, edges), *rows, "seed {seed}");
    }
}

/// The pass must not depend on what it allocated or on what it ran before: two runs over the
/// same graph give the same rows. This is the check that the sweeps' own bookkeeping — the
/// best-order memory, the per-band crossing cache, the buffer the median values are gathered
/// into — leaves nothing behind that changes the next answer.
#[test]
fn two_runs_order_identically() {
    let edges = (0..11u32)
        .flat_map(|i| [(i, (i * 5 + 2) % 11), ((i * 7) % 11, i)])
        .collect::<Vec<_>>();
    assert_eq!(ordered(11, &edges), ordered(11, &edges));
}

#[test]
#[ignore = "debug: is the pass reproducible inside one process"]
fn debug_repeatability() {
    let rows = super::oracle_probe::oracle_digest();
    let mut diff = 0;
    for row in rows.iter().take(80) {
        let count = u32::try_from(row.ranks.len()).expect("u32");
        let a = ordered(count, &row.edges);
        let b = ordered(count, &row.edges);
        if a != b {
            diff += 1;
            eprintln!("seed {} differs between two runs", row.ranks.len());
        }
    }
    eprintln!("{diff} of 80 seeds differ between two runs in one process");
    assert_eq!(diff, 0);
}

#[test]
#[ignore = "debug: who has the better drawing when the orders disagree"]
fn debug_crossing_verdict() {
    use super::oracle_crossings::edge_crossings;
    let rows = super::oracle_probe::oracle_digest();
    let (mut better, mut worse, mut tie, mut agree) = (0, 0, 0, 0);
    for row in &rows {
        let count = u32::try_from(row.ranks.len()).expect("u32");
        let g = super::oracle_probe::ranked_and_ordered(count, &row.edges);
        if super::mincross::crossings::real_ranks(&g) != row.ranks {
            continue;
        }
        let ours = super::mincross::crossings::real_rows(&g);
        let theirs = row.rows();
        if ours == theirs {
            agree += 1;
            continue;
        }
        let a = edge_crossings(&ours, &row.edges);
        let b = edge_crossings(&theirs, &row.edges);
        match a.cmp(&b) {
            std::cmp::Ordering::Less => better += 1,
            std::cmp::Ordering::Greater => worse += 1,
            std::cmp::Ordering::Equal => tie += 1,
        }
    }
    eprintln!("agree {agree}; of the rest: better {better}, worse {worse}, tie {tie}");
}
