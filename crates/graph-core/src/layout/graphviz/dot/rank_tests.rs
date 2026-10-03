//! The rank pass against the oracle: the six closed cases and twenty fixture seeds pinned
//! node by node, plus the edge classification `class2` does next.
//!
//! **Ranks, not coordinates.** The oracle prints a y coordinate; the rank is *derived* from
//! it by `target/probe/rank_oracle.py` as `(y - NODE_H / 2) / (NODE_H + RANKSEP)`, and the
//! probe reports the largest distance any of the 1000 seeds' printed y values sits from that
//! grid. It is **0.0000 of a step over every seed**, so every node of every fixture lands
//! exactly on a rank and the table below is the oracle's own layering rather than a rounded
//! guess at it. Deriving the rank is what makes this test independent of the position pass:
//! nothing here needs an x coordinate, and nothing here is sensitive to the node box width,
//! which is still open.
//!
//! Reproduce the closed cases:
//!
//! ```sh
//! printf 'graph g {\n  n0; n1; n2; n3;\n  n0 -- n1;\n  n1 -- n2;\n  n2 -- n3;\n  n3 -- n0;\n}\n' \
//!     > target/probe/cyc4.dot
//! docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     dot -Tplain target/probe/cyc4.dot
//! ```
//!
//! Reproduce the fixture table (seeds 0 to 19, `n` = 2 to 21):
//!
//! ```sh
//! scripts/orch/gr cargo run -q -p graph-cli --release -- \
//!     emit-graphviz-fixtures --engine twopi --seeds 1000 --out target/dot-probe1000
//! cp target/dot-probe1000/twopi.jsonl target/dotfix/dot.jsonl
//! docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     python3 harness/oracle-graphviz.py target/dotfix dot target/gv-dot-det-a --fixtures=dot.jsonl
//! docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
//!     python3 target/probe/rank_oracle.py --table 20
//! ```
//!
//! Determinism: `rank` is a pure function of the graph, and the two runs in
//! [`two_runs_rank_identically`] are the check that the pass inherits.

use super::oracle_probe::{graph, ranked, ranks_of};
use super::rank::rank;
use super::rank_fixture_edges::FIXTURE_EDGES;

/// One closed case: a name, the input edges, and the rank the oracle gave every node.
type Closed = (&'static str, &'static [(u32, u32)], &'static [i32]);

/// The six closed cases, in the order `docs/measurements/p13-gv2-dot.md` lists them, each
/// with the node's rank derived from the y coordinate `dot -Tplain` printed for it.
///
/// **Read the ranks top-down: the largest y is rank 0.** The 4-cycle is the case that
/// discriminates — `acyclic` must reverse `n3 -> n0`, which puts `n3` on rank 3 and `n0` on
/// rank 0, so a port that broke the cycle the other way round, or that kept the y coordinate
/// instead of inverting it, disagrees on both ends of it.
const CLOSED: &[Closed] = &[
    ("one node", &[], &[0]),
    ("two nodes", &[(0, 1)], &[0, 1]),
    ("3-path", &[(0, 1), (1, 2)], &[0, 1, 2]),
    ("4-cycle", &[(0, 1), (1, 2), (2, 3), (3, 0)], &[0, 1, 2, 3]),
    (
        "5-star",
        &[(0, 1), (0, 2), (0, 3), (0, 4)],
        &[0, 1, 1, 1, 1],
    ),
    (
        "6-branch",
        &[(0, 1), (0, 2), (0, 3), (1, 4), (4, 5)],
        &[0, 1, 1, 1, 2, 3],
    ),
];

#[test]
fn the_six_closed_cases_rank_as_the_oracle_ranks_them() {
    for (name, edges, want) in CLOSED {
        let count = u32::try_from(want.len()).expect("a node count fits u32");
        assert_eq!(&ranked(count, edges), want, "{name}");
    }
}

/// **The negative control for the table above.** Numbering the ranks from the top — which is
/// what a port that kept the y coordinate instead of inverting it would produce — has to
/// fail every case but the one-node one. Without this, a table that only ever compared
/// heights could pass a pass that got the layering upside down.
#[test]
fn the_closed_cases_are_not_mirrored_ranks() {
    let mirrored: Vec<Vec<i32>> = CLOSED
        .iter()
        .map(|(_, _, want)| want.iter().rev().copied().collect())
        .collect();
    for ((name, edges, want), flipped) in CLOSED.iter().zip(&mirrored) {
        let count = u32::try_from(want.len()).expect("a node count fits u32");
        let got = ranked(count, edges);
        if want.len() > 1 {
            assert_ne!(&got, flipped, "{name} must not be its own mirror");
        }
    }
}

/// Twenty fixture seeds, ranks as `target/probe/rank_oracle.py --table 20` read them off the
/// oracle. `n` runs 2 to 21, so this covers the shapes the closed cases do not: a node with
/// three in-edges, a fan of four out of one node, and a graph whose optimal layering needs
/// a pivot rather than a longest path.
const FIXTURES: &[(u32, &[i32])] = &[
    (0, &[1, 0]),
    (1, &[1, 0, 0]),
    (2, &[2, 1, 1, 0]),
    (3, &[2, 1, 1, 1, 0]),
    (4, &[2, 1, 0, 1, 1, 0]),
    (5, &[2, 1, 1, 1, 1, 0, 0]),
    (6, &[2, 1, 1, 1, 1, 0, 0, 1]),
    (7, &[2, 1, 1, 1, 1, 0, 0, 1, 1]),
    (8, &[2, 1, 1, 1, 0, 1, 1, 1, 0, 0]),
    (9, &[3, 2, 2, 2, 1, 1, 2, 1, 0, 1, 2]),
    (10, &[2, 1, 1, 1, 1, 0, 1, 1, 1, 0, 0, 0]),
    (11, &[3, 2, 2, 1, 2, 2, 2, 1, 1, 1, 2, 1, 0]),
    (12, &[4, 3, 3, 3, 2, 3, 3, 3, 2, 2, 2, 1, 1, 0]),
    (13, &[3, 2, 2, 1, 2, 2, 2, 1, 1, 1, 2, 1, 0, 2, 1]),
    (14, &[4, 3, 3, 3, 2, 3, 3, 3, 2, 2, 2, 1, 1, 0, 1, 2]),
    (15, &[3, 2, 2, 2, 2, 2, 1, 1, 1, 2, 1, 0, 2, 1, 1, 0, 1]),
    (16, &[4, 3, 3, 2, 3, 3, 3, 2, 2, 2, 1, 1, 0, 2, 1, 1, 2, 1]),
    (
        17,
        &[3, 2, 2, 2, 1, 1, 1, 1, 2, 1, 0, 2, 0, 1, 0, 1, 0, 1, 0],
    ),
    (
        18,
        &[3, 2, 2, 1, 1, 1, 2, 0, 1, 2, 0, 1, 2, 1, 0, 0, 2, 1, 0, 1],
    ),
    (
        19,
        &[
            6, 5, 4, 2, 1, 4, 1, 4, 5, 4, 0, 5, 0, 4, 0, 1, 3, 4, 3, 4, 3,
        ],
    ),
];

#[test]
fn the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them() {
    assert_eq!(FIXTURE_EDGES.len(), FIXTURES.len());
    for ((seed, edges), (want_seed, want)) in FIXTURE_EDGES.iter().zip(FIXTURES) {
        assert_eq!(
            seed, want_seed,
            "the two tables list the same seeds in order"
        );
        let count = u32::try_from(want.len()).expect("a node count fits u32");
        assert_eq!(&ranked(count, edges), *want, "seed {seed}");
    }
}

/// A pass that reads a clock, a hash order or an allocator's addresses would not give the
/// same answer twice, and neither would one whose walk order depended on a `Vec`'s spare
/// capacity. The rank pass is the first in the pipeline to build enough state for that to
/// show, so the check is here rather than in the last pass that would catch it.
#[test]
fn two_runs_rank_identically() {
    let edges = (0..9)
        .flat_map(|i: u32| [(i, (i * 7 + 3) % 9), ((i * 3) % 9, i)])
        .collect::<Vec<_>>();
    let mut first = graph(9, &edges);
    let mut second = graph(9, &edges);
    rank(&mut first).expect("connected after cycle breaking");
    rank(&mut second).expect("connected after cycle breaking");
    assert_eq!(ranks_of(&first), ranks_of(&second));
}

/// The rank pass leaves the fast graph empty, which is the boundary `class2` starts from:
/// everything the position pass walks is a chain `class2` builds afterwards, and the input
/// edges survive — in `orig_out` and in `edges`, not in the adjacency lists.
#[test]
fn cleanup_leaves_the_input_edges_and_empties_the_adjacency() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3), (3, 0)]);
    rank(&mut g).expect("connected after cycle breaking");
    assert!(g.out.iter().all(|list| list.is_empty()), "out");
    assert!(g.inn.iter().all(|list| list.is_empty()), "inn");
    assert_eq!(g.orig_out[0], vec![0], "the input edges are still there");
    assert_eq!(g.orig_out[3], vec![3]);
    assert!(
        g.edges.iter().all(|e| !e.live),
        "and none of them is in the fast graph any more"
    );
}
