//! Shared scaffolding for the rank pass's oracle tests, and the 1000-seed sweep.
//!
//! **The probe file is not in the repository.** `target/probe/rank1000.txt` is written by
//! `target/probe/rank_oracle.py` inside `ge-graphviz-oracle`, from the same fixture set the
//! twenty seeds in `rank_tests.rs` come from, and the sweep below is `#[ignore]`d because a
//! clean checkout has no `target/`. It asserts three counts rather than printing them, so a
//! regression is a failure and a number nobody reads is not a pass. The command and its
//! output are in `docs/measurements/p13-gv2-dot.md`.
//!
//! Determinism: the sweep is 1000 independent runs of a pure function, so its result is a
//! number and not a sample.

use super::fast::Fast;
use super::rank::rank;
use super::{add_edges, empty_graph};

/// A graph with `count` nodes and `edges`, every node on Graphviz's default box.
pub fn graph(count: u32, edges: &[(u32, u32)]) -> Fast {
    let mut g = empty_graph(count);
    add_edges(&mut g, edges);
    g
}

/// The rank of every real node, in dense-index order.
pub fn ranks_of(g: &Fast) -> Vec<i32> {
    g.nodes.iter().map(|n| n.rank).collect()
}

/// Run the rank pass and hand back the ranks.
pub fn ranked(count: u32, edges: &[(u32, u32)]) -> Vec<i32> {
    let mut g = graph(count, edges);
    rank(&mut g).expect("the fixture graphs are connected and acyclic after the pass");
    ranks_of(&g)
}

/// One row of `target/probe/rank1000.txt`: the oracle's answer for one seed.
pub struct OracleRow {
    /// The fixture edges, as (tail, head).
    pub edges: Vec<(u32, u32)>,
    /// The rank the oracle gave every node.
    pub ranks: Vec<i32>,
}

/// The oracle's per-seed layering, as `target/probe/rank_oracle.py --digest` wrote it:
/// `seed n t,h ... rank rank ...`, one line per seed, the edges before the ranks.
///
/// The file is a probe under `target/`, so it is absent from a clean checkout; the one test
/// that reads it, the sweep below, is `#[ignore]`d for that reason. It is produced by
/// `python3 target/probe/rank_oracle.py --digest target/probe/rank1000.txt` inside
/// `ge-graphviz-oracle`, from the same fixture set as the twenty seeds in `rank_tests.rs`.
pub fn oracle_digest() -> Vec<OracleRow> {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/probe/rank1000.txt");
    let Ok(text) = std::fs::read_to_string(&path) else {
        panic!(
            "{} is missing; see this module's doc for the command that writes it",
            path.display()
        );
    };
    let mut rows = Vec::new();
    for line in text.lines().filter(|l| !l.starts_with('#')) {
        let mut fields = line.split_whitespace();
        let _seed: u32 = fields.next().expect("a seed").parse().expect("a seed");
        let count: usize = fields
            .next()
            .expect("a node count")
            .parse()
            .expect("a count");
        let rest: Vec<&str> = fields.collect();
        let edges: Vec<(u32, u32)> = rest[..rest.len() - count]
            .iter()
            .map(|pair| {
                let (tail, head) = pair.split_once(',').expect("a tail,head pair");
                (tail.parse().expect("a tail"), head.parse().expect("a head"))
            })
            .collect();
        let ranks: Vec<i32> = rest[rest.len() - count..]
            .iter()
            .map(|r| r.parse().expect("a rank"))
            .collect();
        rows.push(OracleRow { edges, ranks });
    }
    assert!(!rows.is_empty(), "the digest has rows");
    rows
}

/// The 1000-seed rank agreement, which is what `docs/measurements/p13-gv2-dot.md` records.
///
/// Ignored by default because it needs the probe files, which are not in the repository; run
/// it with
/// `cargo test -p graph-core --lib -- --ignored rank_agreement_over_1000_seeds` once
/// `target/probe/rank1000.txt` exists.
#[test]
#[ignore = "needs target/probe/rank1000.txt, written by the oracle probe"]
fn rank_agreement_over_1000_seeds() {
    let rows = oracle_digest();
    let mut agree = 0usize;
    let mut equal_cost = 0usize;
    let mut worse = 0usize;
    for row in &rows {
        let count = u32::try_from(row.ranks.len()).expect("a node count fits u32");
        let got = ranked(count, &row.edges);
        if got == row.ranks {
            agree += 1;
        }
        let ours = cost(&acyclic_edges(count, &row.edges), &got);
        let theirs = cost(&acyclic_edges(count, &row.edges), &row.ranks);
        if ours == theirs {
            equal_cost += 1;
        }
        if ours > theirs {
            worse += 1;
        }
    }
    eprintln!(
        "{} of {} seeds agree node for node; {} have equal cost; {} are worse",
        agree,
        rows.len(),
        equal_cost,
        worse
    );
    assert_eq!(equal_cost, RECORDED_EQUAL_COST, "seeds with an equal cost");
    assert_eq!(worse, RECORDED_WORSE, "seeds the port ranks worse on");
    assert_eq!(agree, RECORDED_AGREEMENT, "node-for-node agreement");
}

/// The three numbers `docs/measurements/p13-gv2-dot.md` records for the 1000-seed sweep.
/// They are named here so the assertions above say which measurement they are checking
/// against, and so a change in any of them is a change someone has to look at.
const RECORDED_AGREEMENT: usize = 692;
const RECORDED_EQUAL_COST: usize = 993;
const RECORDED_WORSE: usize = 6;

/// Total weighted edge length of a ranking: what the network simplex minimises, so two
/// rankings with the same cost are two answers to the same question and the port is not
/// *worse* for answering differently.
///
/// Measured on the graph the simplex actually ranked, which is [`acyclic_edges`] and not the
/// input: `acyclic` reverses the back edge of every cycle, and a reversed edge's span is
/// measured the other way round. Comparing costs against the input edge list instead makes
/// a correct ranking look worse than the oracle's on every graph with a cycle in it, which
/// is what the first version of this test did.
fn cost(edges: &[(u32, u32, i64)], ranks: &[i32]) -> i64 {
    edges
        .iter()
        .map(|&(t, h, w)| w * (i64::from(ranks[h as usize] - ranks[t as usize]) - 1).max(0))
        .sum()
}

/// The edge list the rank pass sees: `class1` then `acyclic`, as `(tail, head, weight)`.
/// The weight matters because parallel input edges are folded together before ranking.
fn acyclic_edges(count: u32, edges: &[(u32, u32)]) -> Vec<(u32, u32, i64)> {
    let mut g = graph(count, edges);
    super::class1::run(&mut g);
    for component in super::decomp::decompose(&g) {
        super::acyclic::run(&mut g, &component);
    }
    g.edges
        .iter()
        .filter(|e| e.live)
        .map(|e| (e.tail, e.head, i64::from(e.weight)))
        .collect()
}
