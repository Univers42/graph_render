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

use super::fast::{Fast, Kind};
use super::mincross::{self, crossings};
use super::oracle_crossings::edge_crossings as crossings_of;
use super::oracle_digest::{OracleRow, oracle_digest};
use super::position::position;
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

/// Every node's id as the oracle's fixtures name them: `n0`, `n1`, … `n{count - 1}`.
///
/// This is the one place the port turns a dense index into text, and it is why the size of a
/// node's box is the measured table's rather than a constant: the reference sizes a node from
/// its rendered label, and the label is the id.
pub fn fixture_ids(count: u32) -> Vec<String> {
    (0..count).map(|i| format!("n{i}")).collect()
}

/// Run all three passes over a graph whose nodes are named `n0`…`n{count - 1}`, and hand back
/// the graph the position pass left: every node's centre, in the frame `-Tplain` prints.
pub fn positioned(count: u32, edges: &[(u32, u32)]) -> Fast {
    let ids: Vec<String> = fixture_ids(count);
    let borrowed: Vec<&str> = ids.iter().map(String::as_str).collect();
    positioned_over(super::build(&borrowed, edges))
}

/// The three passes over a graph already built, so the sized and the default-box variants differ
/// in nothing but the box.
fn positioned_over(mut g: Fast) -> Fast {
    rank(&mut g).expect("the fixture graphs are connected and acyclic after the pass");
    mincross::run(&mut g);
    positioned_after(g)
}

/// The position pass alone, over a graph the rank and mincross passes have already run — the
/// form the sweeps need, so the graph whose order they compare is the graph they place.
pub fn positioned_after(mut g: Fast) -> Fast {
    position(&mut g).expect("the fixture graphs are connected after the pass");
    g
}

/// This crate's copy of the plain format's number formatter: `%g` at five significant digits,
/// trailing zeros stripped (`lib/common/output.c`'s `printdouble`).
///
/// Reproducing the oracle's formatter rather than rounding to a chosen number of decimals is
/// what makes "byte for byte" checkable: the grid moves with the magnitude, so a fixed decimal
/// count is wrong at both ends of a drawing.
pub fn plain_g(inches: f64) -> String {
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

/// A drawing's centres as the plain format would print them: the x column and the y column, each
/// node's own five significant digits in inches, space separated.
pub fn printed_columns(g: &Fast) -> (String, String) {
    let column = |pick: fn(&(f64, f64)) -> f64| {
        centres(g)
            .iter()
            .map(|p| plain_g(pick(p) / POINTS_PER_INCH))
            .collect::<Vec<_>>()
            .join(" ")
    };
    (column(|p| p.0), column(|p| p.1))
}

/// `POINTS_PER_INCH` (`lib/common/geom.h:58`): the layout computes in points and the plain
/// format prints in inches.
pub const POINTS_PER_INCH: f64 = 72.0;

/// The centre of every **real** node, in the frame `-Tplain` prints, in dense-index order.
///
/// Chain dummies are dropped: the plain format prints no dummy, so there is nothing on the
/// oracle's side to compare one against.
pub fn centres(g: &Fast) -> Vec<(f64, f64)> {
    let count = g.nodes.iter().filter(|n| n.kind == Kind::Normal).count() as u32;
    (0..count)
        .map(|n| (g.nodes[n as usize].coord.x, g.nodes[n as usize].coord.y))
        .collect()
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
    let tally = sweep_order(&rows);
    eprintln!(
        "{} of {} seeds agree on the ranks; of those {} on every rank's order, {} on the \
         crossing count. Of the {} disagreements: {} keep the crossing count, {} draw fewer \
         crossings and {} draw more",
        tally.same_ranks,
        rows.len(),
        tally.same_order,
        tally.same_crossings,
        tally.same_ranks - tally.same_order,
        tally.tied,
        tally.better,
        tally.worse,
    );
    assert_eq!(
        tally.same_ranks, RECORDED_RANK_AGREEMENT,
        "seeds whose ranks agree"
    );
    assert_eq!(
        tally.same_order, RECORDED_ORDER_AGREEMENT,
        "seeds whose order agrees"
    );
    assert_eq!(
        tally.same_crossings, RECORDED_CROSSING_AGREEMENT,
        "seeds whose crossing count agrees"
    );
}

/// Count the order agreement over every row of the digest, one pass and no allocation per
/// seed beyond the graph itself.
fn sweep_order(rows: &[OracleRow]) -> Tally {
    let mut tally = Tally::default();
    for row in rows {
        let g = tally.graph(row);
        if crossings::real_ranks(&g) != row.ranks {
            continue;
        }
        tally.same_ranks += 1;
        tally.grade(row, &g);
    }
    tally
}

/// What the order sweep counts, and how a disagreement is graded.
///
/// The grade is the one question a disagreement raises: is the port's drawing a different
/// arrangement of the same drawing, a better one, or a worse one? Three counters, and the
/// crossing count is the same [`edge_crossings`](super::oracle_crossings::edge_crossings)
/// on both sides.
#[derive(Default)]
struct Tally {
    same_ranks: usize,
    same_order: usize,
    same_crossings: usize,
    tied: usize,
    better: usize,
    worse: usize,
}

impl Tally {
    /// One seed that agrees on its ranks: compare the port's rows with the oracle's, and
    /// grade a disagreement by which of the two drawings has fewer crossings.
    fn grade(&mut self, row: &OracleRow, g: &Fast) {
        let ours = crossings::real_rows(g);
        let theirs = row.rows();
        if ours == theirs {
            self.same_order += 1;
        }
        let ours_cross = crossings_of(&ours, &row.edges);
        let theirs_cross = crossings_of(&theirs, &row.edges);
        if ours_cross == theirs_cross {
            self.same_crossings += 1;
        }
        if ours != theirs {
            match ours_cross.cmp(&theirs_cross) {
                std::cmp::Ordering::Less => self.better += 1,
                std::cmp::Ordering::Greater => self.worse += 1,
                std::cmp::Ordering::Equal => self.tied += 1,
            }
        }
    }

    /// The port's own run of one fixture seed, kept on the tally so [`Tally::grade`] reads
    /// one row and one graph.
    fn graph(&self, row: &OracleRow) -> Fast {
        let count = u32::try_from(row.ranks.len()).expect("a node count fits u32");
        ranked_and_ordered(count, &row.edges)
    }
}

/// The three numbers `docs/measurements/p13-gv2-dot.md` records for the order sweep, as the
/// run quoted there measured them. They are named so the assertions above say which
/// measurement they check, and so a change in any of them is a change someone must look at.
const RECORDED_RANK_AGREEMENT: usize = 692;
const RECORDED_ORDER_AGREEMENT: usize = 408;
const RECORDED_CROSSING_AGREEMENT: usize = 446;

/// The 1000-seed position agreement, the third half of the same measurement.
///
/// **The population is the seeds whose order already agrees in every rank**, which is the 408
/// [`order_agreement_over_1000_seeds`] measures and not all 1000: a coordinate is only
/// comparable when the row it sits on is the same row. The question it answers is the strictest
/// one the oracle's own output allows — *every node's centre, printed*, five significant digits
/// in inches — because `-Tplain` prints nothing finer and a comparison at a finer grid would be
/// against digits it never printed.
///
/// Ignored by default because it needs the probe file; run it with `cargo test -p graph-core
/// --lib -- --ignored position_agreement_over_1000_seeds` once `target/probe/dot1000.txt` exists.
#[test]
#[ignore = "needs target/probe/dot1000.txt, written by the oracle probe"]
fn position_agreement_over_1000_seeds() {
    let rows = oracle_digest();
    let tally = sweep_position(&rows);
    eprintln!(
        "{} of {} seeds agree on every rank's order; of those {} print every node centre \
         exactly as the oracle prints it",
        tally.comparable,
        rows.len(),
        tally.exact
    );
    eprintln!("exact seeds: {:?}", tally.exact_seeds);
    assert_eq!(
        tally.comparable, RECORDED_ORDER_AGREEMENT,
        "comparable seeds"
    );
    assert_eq!(tally.exact, RECORDED_POSITION_AGREEMENT, "exact placements");
}

/// The position sweep's two counts and the seeds behind the second.
#[derive(Default)]
struct PositionTally {
    /// Seeds whose every rank's order already agrees: the only comparable ones.
    comparable: usize,
    /// Those that also place every node's centre exactly as the oracle prints it.
    exact: usize,
    /// Which seeds those are, so a regression names them.
    exact_seeds: Vec<u32>,
}

impl PositionTally {
    /// One seed whose order already agrees: place it, and compare the printed columns against
    /// the oracle's own printed strings.
    fn grade(&mut self, row: &OracleRow, g: Fast) {
        self.comparable += 1;
        let placed = positioned_after(g);
        if printed_columns(&placed) == (row.xs.join(" "), row.ys.join(" ")) {
            self.exact += 1;
            self.exact_seeds.push(row.seed);
        }
    }
}

/// Count the position agreement over every row of the digest whose order already agrees.
fn sweep_position(rows: &[OracleRow]) -> PositionTally {
    let mut tally = PositionTally::default();
    for row in rows {
        let g = ranked_and_ordered(count_of(row), &row.edges);
        if crossings::real_rows(&g) != row.rows() {
            continue;
        }
        tally.grade(row, g);
    }
    tally
}

/// A row's node count, as the `u32` the passes take.
fn count_of(row: &OracleRow) -> u32 {
    u32::try_from(row.ranks.len()).expect("a node count fits u32")
}

/// How many of the order-agreeing seeds the position pass places exactly as the oracle prints
/// them, as `docs/measurements/p13-gv2-dot.md`'s "Position" section records the run measuring.
const RECORDED_POSITION_AGREEMENT: usize = 10;
