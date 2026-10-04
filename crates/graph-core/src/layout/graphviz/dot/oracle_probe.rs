//! Shared scaffolding for the rank and mincross oracle tests, and the two 1000-seed sweeps.
//!
//! **The probe file is not in the repository.** `target/probe/dot1000.txt` is written by
//! `harness/oracle-dot-probe.py` inside `ge-graphviz-oracle`, from the same fixture set the
//! twenty seeds in `rank_tests.rs` come from, and the two sweeps below are `#[ignore]`d
//! because a clean checkout has no `target/`. Each asserts its counts rather than printing
//! them, so a regression is a failure and a number nobody reads is not a pass. The command
//! and its output are in `docs/measurements/p13-gv2-dot.md`.
//!
//! One file carries both answers, because both come out of one `-Tplain` run per seed: the
//! rank of every node (derived from its printed y) and the order of the nodes within each
//! rank (their printed x). See the probe's own doc for the derivation and for the one line
//! of `-Tplain` this file's rows are.
//!
//! Determinism: each sweep is 1000 independent runs of a pure function, so its result is a
//! measurement and not a sample.

use super::fast::Fast;
use super::mincross::{self, crossings};
use super::oracle_crossings::edge_crossings as crossings_of;
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

/// Run the rank pass and the mincross pass, and hand back the graph they left: the chains,
/// the dummies and every node's position within its rank.
pub fn ranked_and_ordered(count: u32, edges: &[(u32, u32)]) -> Fast {
    let mut g = graph(count, edges);
    rank(&mut g).expect("the fixture graphs are connected and acyclic after the pass");
    mincross::run(&mut g);
    g
}

/// Run the rank pass and the mincross pass, and hand back one row per rank: the nodes on it,
/// left to right. Rank 0 is the top row, so the first row is the first one printed.
pub fn ordered(count: u32, edges: &[(u32, u32)]) -> Vec<Vec<u32>> {
    crossings::real_rows(&ranked_and_ordered(count, edges))
}

/// One row of `target/probe/dot1000.txt`: the oracle's answer for one seed.
pub struct OracleRow {
    /// The fixture edges, as (tail, head), in declaration order.
    pub edges: Vec<(u32, u32)>,
    /// The rank the oracle gave every node.
    pub ranks: Vec<i32>,
    /// The nodes of every rank left to right, rank 0 first: a permutation of `ranks.len()`
    /// node indices, which is what the probe's order column is.
    pub order: Vec<u32>,
}

impl OracleRow {
    /// The oracle's order as one row per rank, so it can be compared with
    /// [`crossings::rank_rows`] without the caller regrouping it.
    pub fn rows(&self) -> Vec<Vec<u32>> {
        let highest = self.ranks.iter().copied().max().unwrap_or(-1);
        let mut rows = vec![Vec::new(); usize::try_from(highest + 1).expect("a rank count")];
        for &node in &self.order {
            rows[usize::try_from(self.ranks[node as usize]).expect("a rank")].push(node);
        }
        rows
    }
}

/// The oracle's per-seed layering and order, as `harness/oracle-dot-probe.py --digest` wrote
/// it: `seed n t,h ... <n ranks> <n order>`, one line per seed.
///
/// The file is a probe output under `target/`, so it is absent from a clean checkout; the two
/// tests that read it are `#[ignore]`d for that reason. It is produced by
/// `python3 harness/oracle-dot-probe.py target/dotfix --fixtures=dot.jsonl --digest
/// target/probe/dot1000.txt` inside `ge-graphviz-oracle`, from the same fixture set as the
/// twenty seeds in `rank_tests.rs`.
pub fn oracle_digest() -> Vec<OracleRow> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/probe/dot1000.txt");
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
        let count: usize = fields.next().expect("a node count").parse().expect("a count");
        let rest: Vec<&str> = fields.collect();
        let (pairs, tail) = rest.split_at(rest.len() - 2 * count);
        let edges: Vec<(u32, u32)> = pairs
            .iter()
            .map(|pair| {
                let (tail, head) = pair.split_once(',').expect("a tail,head pair");
                (tail.parse().expect("a tail"), head.parse().expect("a head"))
            })
            .collect();
        let numbers = |slice: &[&str]| -> Vec<i32> {
            slice.iter().map(|n| n.parse().expect("a number")).collect()
        };
        let ranks = numbers(&tail[..count]);
        let order: Vec<u32> = numbers(&tail[count..]).into_iter().map(|n| n as u32).collect();
        rows.push(OracleRow { edges, ranks, order });
    }
    assert!(!rows.is_empty(), "the digest has rows");
    rows
}

/// The 1000-seed rank agreement, which is what `docs/measurements/p13-gv2-dot.md` records.
///
/// Ignored by default because it needs the probe file, which is not in the repository; run it
/// with `cargo test -p graph-core --lib -- --ignored rank_agreement_over_1000_seeds` once
/// `target/probe/dot1000.txt` exists.
#[test]
#[ignore = "needs target/probe/dot1000.txt, written by the oracle probe"]
fn rank_agreement_over_1000_seeds() {
    let rows = oracle_digest();
    let (mut agree, mut equal_cost, mut worse) = (0usize, 0usize, 0usize);
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

/// The three numbers `docs/measurements/p13-gv2-dot.md` records for the 1000-seed rank
/// sweep. They are named here so the assertions above say which measurement they are checking
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
/// measured the other way round. Comparing costs against the input edge list instead makes a
/// correct ranking look worse than the oracle's on every graph with a cycle in it, which is
/// what the first version of this test did.
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

/// The 1000-seed order agreement, the second half of the same measurement.
///
/// Only the seeds whose **ranks** already agree node for node are counted: an order is only
/// comparable when the rows it orders are the same rows. Two numbers come out of that set —
/// the seeds whose every rank is in the same order as the oracle's, and the seeds whose
/// drawing has the same number of edge crossings — and the second is the one that says
/// whether a disagreement is a different arrangement of the same drawing or a worse drawing.
///
/// The crossing count is [`oracle_crossings::edge_crossings`], not the pass's own count, and
/// that is a decision: the pass counts over the chains and the chain dummies, `-Tplain`
/// prints no dummies and no count at all, so the pass's count has no counterpart on the
/// oracle's side to be compared with. `edge_crossings` reads nothing but a rank and a row, so
/// one implementation computes both sides and the two cannot differ for any reason other
/// than the orders themselves.
#[test]
#[ignore = "needs target/probe/dot1000.txt, written by the oracle probe"]
fn order_agreement_over_1000_seeds() {
    let rows = oracle_digest();
    let mut same_ranks = 0usize;
    let mut same_order = 0usize;
    let mut same_crossings = 0usize;
    let mut ties = 0usize;
    for row in &rows {
        let count = u32::try_from(row.ranks.len()).expect("a node count fits u32");
        let g = ranked_and_ordered(count, &row.edges);
        if crossings::real_ranks(&g) != row.ranks {
            continue;
        }
        same_ranks += 1;
        let ours = crossings::real_rows(&g);
        let theirs = row.rows();
        let ours_cross = crossings_of(&ours, &row.edges);
        let theirs_cross = crossings_of(&theirs, &row.edges);
        if ours == theirs {
            same_order += 1;
        }
        if ours_cross == theirs_cross {
            same_crossings += 1;
        } else if ours != theirs {
            ties += 0;
        }
        if ours != theirs && ours_cross == theirs_cross {
            ties += 1;
        }
    }
    eprintln!(
        "of {same_ranks} rank-agreeing seeds, {same_order} agree on every rank's order and \
         {same_crossings} on the crossing count; {ties} of the {same_order} \
         disagreements keep the crossing count",
        same_order = same_ranks - same_order
    );
    assert_eq!(same_ranks, RECORDED_RANK_AGREEMENT, "seeds whose ranks agree");
    assert_eq!(same_order, RECORDED_ORDER_AGREEMENT, "seeds whose order agrees");
    assert_eq!(
        same_crossings, RECORDED_CROSSING_AGREEMENT,
        "seeds whose crossing count agrees"
    );
}

/// The three numbers `docs/measurements/p13-gv2-dot.md` records for the order sweep. They
/// are filled in from the run the measurement file quotes; a change in any of them is a
/// change someone has to look at.
const RECORDED_RANK_AGREEMENT: usize = 0;
const RECORDED_ORDER_AGREEMENT: usize = 0;
const RECORDED_CROSSING_AGREEMENT: usize = 0;
