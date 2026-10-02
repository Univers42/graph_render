//! `class2` (`class2.c:155-293`): turn the ranked *input* graph into the graph the next two
//! passes walk — a chain of virtual nodes for every edge that spans more than one rank,
//! merged parallel edges, and the flat and other lists for the edges that span none.
//!
//! It runs after ranking and before `mincross`, and it is where `dot` decides what an edge
//! looks like. Three outcomes, in the reference's order of preference for each edge:
//!
//! - a parallel edge joins the chain its twin built, and its weight and count are folded
//!   into it (`merge_chain`), so two edges between the same pair draw as one;
//! - a same-rank edge becomes *flat*: it stays in the graph at zero rank separation and
//!   goes into `flat_out`/`flat_in` rather than `out`/`inn`;
//! - anything else becomes a chain: one virtual node per intervening rank, each joined by a
//!   virtual edge carrying the original's weight and minimum length.
//!
//! **The input edges are read from `orig_out`, not from `out`.** `cleanup1` emptied both
//! adjacency lists at the end of the rank pass, exactly as the reference does; in the C the
//! input edges still live in the cgraph, which `agfstout` walks, while `ND_out` holds only
//! what `class2` itself has built. `orig_out` is this port's cgraph.
//!
//! Two things drop out because this port has no clusters and no edge labels: `interclrep`
//! and `mergeable`'s label and port comparisons (`ports_eq` is always true here), so a
//! multi-edge pair is always mergeable. The fixtures have no self-loops either, but the
//! self-edge branch is kept because it is the reference's and costs one line.
//!
//! Determinism: the outer walk is the input order (which is the dense node order), the inner
//! walk is declaration order, and `prev` is the previous *surviving* edge of that walk — the
//! reference's `prev`, which is why a multi-edge merges into its twin and not into whichever
//! edge happens to share endpoints.

use super::NODESEP;
use super::fast::{Fast, Kind, Node};

/// `table[3][3]` (`mincross.c:1706-1709`), the `virtual_weight` multiplier by endpoint class:
/// ordinary, singleton, virtual. A virtual node is always its own class, a real node is a
/// singleton when at most one edge touches it.
const VIRTUAL_WEIGHT: [[i32; 3]; 3] = [[1, 1, 1], [1, 2, 2], [1, 2, 4]];

/// `class2`: classify every input edge of `g`, given the ranks the rank pass assigned.
pub fn run(g: &mut Fast) {
    count_weight_classes(g);
    for node in 0..g.nodes.len() as u32 {
        let mut prev: Option<u32> = None;
        for edge in g.orig_out[node as usize].clone() {
            prev = classify(g, edge, prev);
        }
    }
}

/// The first loop of `class2` (`class2.c:166-172`): every node's `ND_weight_class` is how
/// many edge ends touch it, capped at 2. It runs over the *input* edges, before any chain
/// exists, so a virtual node's class is the 0 `virtual_node` left it — its own class — and
/// no edge's weight is ever scaled from a class that a chain changed.
fn count_weight_classes(g: &mut Fast) {
    for node in 0..g.nodes.len() {
        for &edge in &g.orig_out[node] {
            let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
            bump(&mut g.nodes[head as usize].weight_class);
            bump(&mut g.nodes[tail as usize].weight_class);
        }
    }
}

/// `ND_weight_class(n) <= 2 && ND_weight_class(n)++`.
fn bump(class: &mut i32) {
    if *class <= 2 {
        *class += 1;
    }
}

/// One edge's classification, returning the new `prev`: `Some(e)` where the reference sets
/// `prev = e` and continues, `None` where it merged and `continue`s without setting it.
///
/// The order is the reference's (`class2.c:179-284`): already processed, cluster, parallel,
/// self, same-rank, forward, backward.
fn classify(g: &mut Fast, edge: u32, prev: Option<u32>) -> Option<u32> {
    if g.edges[edge as usize].to_virt.is_some() {
        return Some(edge);
    }
    let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
    let twin =
        prev.filter(|&b| tail == g.edges[b as usize].tail && head == g.edges[b as usize].head);
    if let Some(before) = twin {
        if g.nodes[tail as usize].rank == g.nodes[head as usize].rank {
            g.merge_oneway(edge, before);
            g.other_edge(edge);
            return None;
        }
        let chain = g.edges[before as usize]
            .to_virt
            .expect("a processed multi-edge's twin has a chain");
        merge_chain(g, edge, chain);
        g.other_edge(edge);
        return None;
    }
    if tail == head {
        g.other_edge(edge);
        return Some(edge);
    }
    let tr = g.nodes[tail as usize].rank;
    let hr = g.nodes[head as usize].rank;
    if tr == hr {
        g.flat_edge(edge);
        return Some(edge);
    }
    if hr > tr {
        make_chain(g, tail, head, edge);
        return Some(edge);
    }
    shadow(g, edge, tail, head)
}

/// The backward-edge branch (`class2.c:256-284`): an edge that runs *up* a rank, which
/// `acyclic` left behind because its reversal had nothing to merge into. Where a downward
/// twin exists the edge is drawn as the twin's other side and merged into its chain;
/// otherwise it gets a chain of its own, from its head down to its tail.
fn shadow(g: &mut Fast, edge: u32, tail: u32, head: u32) -> Option<u32> {
    for &twin in &g.orig_out[head as usize].clone() {
        // `ED_edge_type(opp) == IGNORED` is the `concentrate` attribute's mark and no port
        // reads it, so the third condition of `class2.c:260` drops out with it.
        if g.edges[twin as usize].head != tail || g.edges[twin as usize].head == head {
            continue;
        }
        if g.edges[twin as usize].to_virt.is_none() {
            let (t, h) = (g.edges[twin as usize].tail, g.edges[twin as usize].head);
            make_chain(g, t, h, twin);
        }
        let chain = g.edges[twin as usize]
            .to_virt
            .expect("the twin was just chained");
        g.other_edge(edge);
        merge_chain(g, edge, chain);
        return None;
    }
    make_chain(g, head, tail, edge);
    Some(edge)
}

/// `make_chain` (`class2.c:69-96`): one virtual node per rank between `from` and `to`, each
/// joined to the last by a virtual edge carrying the original's weight, count, penalty and
/// minimum length, each then re-weighted by [`virtual_weight`]. `to` is used as the last
/// hop rather than a virtual node, so an edge spanning two ranks gets exactly one dummy.
fn make_chain(g: &mut Fast, from: u32, to: u32, orig: u32) {
    let mut at = from;
    for r in (g.nodes[from as usize].rank + 1)..=g.nodes[to as usize].rank {
        let next = if r < g.nodes[to as usize].rank {
            let dummy = g.add_node(Node::virtual_node(NODESEP));
            g.nodes[dummy as usize].rank = r;
            dummy
        } else {
            to
        };
        let link = g.add_chain(at, next, orig);
        virtual_weight(g, link);
        at = next;
    }
}

/// `merge_chain` (`class2.c:130-148`): fold `edge`'s count, penalty and weight into the
/// chain `chain` and into every link of it, stopping at the chain's last rank. The links
/// below that rank also grow by `nodesep / 2` on each side, because a merged edge is drawn
/// as two edges sharing the dummies between them.
fn merge_chain(g: &mut Fast, edge: u32, chain: u32) {
    let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
    let last_rank = g.nodes[tail as usize].rank.max(g.nodes[head as usize].rank);
    let (count, penalty, weight) = {
        let source = &g.edges[edge as usize];
        (source.count, source.xpenalty, source.weight)
    };
    let mut rep = chain;
    loop {
        {
            let target = &mut g.edges[rep as usize];
            target.count += count;
            target.xpenalty += penalty;
            target.weight += weight;
        }
        let end = g.edges[rep as usize].head;
        if g.nodes[end as usize].rank == last_rank {
            return;
        }
        incr_width(g, end);
        let Some(&next) = g.out[end as usize].first() else {
            return;
        };
        rep = next;
    }
}

/// `incr_width` (`class2.c:41-47`): `nodesep / 2` points on each side, integer division as
/// the reference's `int`.
fn incr_width(g: &mut Fast, node: u32) {
    let half = (NODESEP / 2.0).trunc();
    let record = &mut g.nodes[node as usize];
    record.lw += half;
    record.rw += half;
}

/// `virtual_weight` (`mincross.c:1719-1731`) over `endpoint_class` (`mincross.c:1711-1717`):
/// the link's weight times the table entry for its two endpoint classes. An edge between two
/// ordinary nodes keeps its weight; one that touches a singleton doubles, because that node
/// has nowhere else to go; one between two virtual nodes quadruples, because the dummies are
/// only there for this edge.
fn virtual_weight(g: &mut Fast, edge: u32) {
    let (tail, head) = (g.edges[edge as usize].tail, g.edges[edge as usize].head);
    let factor = VIRTUAL_WEIGHT[class_of(g, tail)][class_of(g, head)];
    g.edges[edge as usize].weight *= factor;
}

/// `endpoint_class`: 0 ordinary, 1 singleton, 2 virtual.
fn class_of(g: &Fast, node: u32) -> usize {
    let record = &g.nodes[node as usize];
    if record.kind == Kind::Virtual {
        2
    } else if record.weight_class <= 1 {
        1
    } else {
        0
    }
}
