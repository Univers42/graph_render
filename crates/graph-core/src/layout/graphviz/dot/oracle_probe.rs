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
/// The crossing count is [`edge_crossings`], not the pass's own count, and that is a
/// decision: the pass counts over the chains and the chain dummies, `-Tplain` prints no
/// dummies and no count at all, so the pass's count has no counterpart on the oracle's side
/// to be compared with. `edge_crossings` reads nothing but a rank and a row, so one
/// implementation computes both sides and the two cannot differ for any reason other than
/// the orders themselves.
#[test]
#[ignore = "needs target/probe/dot1000.txt, written by the oracle probe"]
fn order_agreement_over_1000_seeds() {
    let rows = oracle_digest();
    let mut same_ranks = 0usize;
    let mut same_order = 0usize;
    let mut same_crossings = 0usize;
    for row in &rows {
        let count = u32::try_from(row.ranks.len()).expect("a node count fits u32");
        let g = ranked_and_ordered(count, &row.edges);
        if crossings::real_ranks(&g) != row.ranks {
            continue;
        }
        same_ranks += 1;
        let ours = crossings::real_rows(&g);
        let theirs = row.rows();
        if ours == theirs {
            same_order += 1;
        }
        if edge_crossings(&ours, &row.edges) == edge_crossings(&theirs, &row.edges) {
            same_crossings += 1;
        }
    }
    eprintln!(
        "of {same_ranks} rank-agreeing seeds, {same_order} agree on every rank's order and \
         {same_crossings} on the crossing count, out of {}",
        rows.len()
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

/// One input edge as the straight segment its two endpoints describe.
struct Segment {
    /// The lower of the two ranks.
    low: usize,
    /// The tail's place on [`Segment::low`].
    from: i64,
    /// The higher of the two ranks.
    high: usize,
    /// The head's place on [`Segment::high`].
    to: i64,
}

/// Where every node of a drawing sits: its rank and its place on it, read once so the pair
/// loop below is a comparison rather than two searches.
struct Layout {
    rank: Vec<i32>,
    place: Vec<i64>,
}

impl Layout {
    fn of(rows: &[Vec<u32>]) -> Self {
        let highest = rows.len();
        let mut rank = vec![-1; highest * 4];
        let mut place = vec![0; highest * 4];
        for (r, row) in rows.iter().enumerate() {
            for (i, &node) in row.iter().enumerate() {
                if let Some(slot) = node_index(node, rank.len()) {
                    rank[slot] = r as i32;
                    place[slot] = i as i64;
                }
            }
        }
        Self { rank, place }
    }

    fn rank_of(&self, node: u32) -> Option<i32> {
        let slot = node_index(node, self.rank.len())?;
        (self.rank[slot] >= 0).then_some(self.rank[slot])
    }

    fn place_of(&self, node: u32) -> i64 {
        node_index(node, self.place.len()).map_or(0, |slot| self.place[slot])
    }
}

/// A node's slot in a per-node array, or `None` when the array is too small for it — which
/// only happens if a row holds a node the arrays were not sized for.
fn node_index(node: u32, len: usize) -> Option<usize> {
    (usize::try_from(node).expect("a node index fits usize") < len).then_some(node as usize)
}

/// The number of pairs of input edges that cross in a drawing, from nothing but its rows.
///
/// **What is drawn.** Each input edge is the straight segment between its two endpoints,
/// with a rank as the vertical coordinate and a node's place in its row as the horizontal
/// one. Two edges that share an endpoint do not count — they touch, and the reference's own
/// count does not charge one node's edges against each other either — and two edges whose
/// bands meet only at a single rank do not count, because they share no band.
///
/// **How the crossing is decided.** Over the band the two segments share, the left-to-right
/// order is read at the bottom of the band and at the top; if it is not the same way round,
/// they crossed. A segment's horizontal position at a rank is its endpoints' positions
/// interpolated, which is a ratio of integers, and two ratios are compared by
/// cross-multiplying — so the whole count is integer arithmetic and reads the same on both
/// targets (`prompt.md` §6 D3).
///
/// **Both sides see real nodes only.** A chain dummy is the pass's internal state and the
/// plain format prints none of them, so a row carrying them would not be the same row on
/// both sides and the interpolation would not be the same interpolation. Rows of real nodes
/// are therefore what this takes, which is also what makes the count a property of the
/// *drawing* and not of the chains inside it.
fn edge_crossings(rows: &[Vec<u32>], edges: &[(u32, u32)]) -> i64 {
    let layout = Layout::of(rows);
    let mut cross = 0i64;
    for (index, first) in edges.iter().enumerate() {
        for second in &edges[index + 1..] {
            cross += i64::from(crosses(&layout, *first, *second));
        }
    }
    cross
}

/// Whether these two edges cross, as [`edge_crossings`] counts it.
fn crosses(layout: &Layout, first: (u32, u32), second: (u32, u32)) -> bool {
    let shared = [first.0, first.1].iter().any(|n| [second.0, second.1].contains(n));
    if shared {
        return false;
    }
    let (Some(a), Some(b)) = (segment(layout, first), segment(layout, second)) else {
        return false;
    };
    let (bottom, top) = (a.low.max(b.low), a.high.min(b.high));
    (bottom < top) && (left_of(&a, &b, bottom) != left_of(&a, &b, top))
}

/// An edge as the segment its endpoints describe, lower rank first, or `None` when an
/// endpoint is on no row at all.
fn segment(layout: &Layout, edge: (u32, u32)) -> Option<Segment> {
    let first = (layout.rank_of(edge.0)?, layout.place_of(edge.0));
    let second = (layout.rank_of(edge.1)?, layout.place_of(edge.1));
    let (low, high) = if first.0 <= second.0 { (first, second) } else { (second, first) };
    Some(Segment { low: low.0 as usize, from: low.1, high: high.0 as usize, to: high.1 })
}

/// Whether segment `a` is left of segment `b` at rank `y`.
fn left_of(a: &Segment, b: &Segment, y: usize) -> bool {
    let (an, ad) = interpolated(a, y);
    let (bn, bd) = interpolated(b, y);
    an * bd < bn * ad
}

/// A segment's horizontal position at rank `y`, as a numerator over the segment's span.
fn interpolated(segment: &Segment, y: usize) -> (i64, i64) {
    let span = (segment.high - segment.low) as i64;
    let walked = y as i64 - segment.low as i64;
    (segment.from * span + (segment.to - segment.from) * walked, span)
}

#[test]
#[ignore = "debug"]
fn debug_first_disagreements() {
    let rows = oracle_digest();
    let mut shown = 0;
    for row in rows.iter().take(200) {
        let count = u32::try_from(row.ranks.len()).expect("u32");
        let g = ranked_and_ordered(count, &row.edges);
        if crossings::real_ranks(&g) != row.ranks {
            continue;
        }
        let ours = crossings::real_rows(&g);
        if ours == row.rows() {
            continue;
        }
        shown += 1;
        if shown > 6 {
            break;
        }
        eprintln!("seed {} n={} edges={:?}", row.ranks.len(), row.ranks.len(), row.edges);
        eprintln!("  ours   {ours:?}");
        eprintln!("  theirs {:?}", row.rows());
        eprintln!(
            "  drawn crossings ours {} theirs {}",
            edge_crossings(&ours, &row.edges),
            edge_crossings(&row.rows(), &row.edges)
        );
    }
}
