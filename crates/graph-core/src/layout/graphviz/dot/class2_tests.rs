//! `class2`'s three outcomes, checked against what `class2.c` says it does rather than
//! against a coordinate: the pass produces no x yet.
//!
//! The reference's per-edge preference is the order these tests take — merged parallel
//! edges, then *flat* same-rank edges, then a chain of virtual nodes for everything else —
//! and each one gets its own test because each is a branch a port can lose without any
//! coordinate changing.
//!
//! Two of the five put ranks side by side by hand, and say so: with `dot`'s own
//! `ED_minlen` of 1 a plain acyclic graph never produces a same-rank edge, so the flat
//! branch is otherwise unreachable from a fixture.
//!
//! Determinism: `class2` reads the *input* edges in declaration order and never revisits an
//! edge, so the chains it builds are a function of the input order alone.

use super::fast::Kind;
use super::rank::rank;
use super::{class2, graph, ranks_of};

/// An edge spanning `k` ranks gets `k - 1` dummies, one per intervening rank, joined by `k`
/// links. A four-node path with a shortcut from end to end is the shape: ranks 0 to 3, so
/// `n0 -> n3` gets two dummies and each of the three short edges none.
#[test]
fn class2_chains_a_long_edge() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
    rank(&mut g).expect("connected after cycle breaking");
    assert_eq!(ranks_of(&g), vec![0, 1, 2, 3]);
    let before = g.nodes.len() as u32;
    class2::run(&mut g);
    assert_eq!(g.nodes.len() as u32 - before, 2, "one dummy per intervening rank");
    let dummies: Vec<u32> = (before..g.nodes.len() as u32).collect();
    let ranks: Vec<i32> = dummies.iter().map(|&n| g.nodes[n as usize].rank).collect();
    assert_eq!(ranks, vec![1, 2], "on the ranks between the ends");
    for &dummy in &dummies {
        assert_eq!(g.nodes[dummy as usize].kind, Kind::Virtual);
        assert_eq!(g.inn[dummy as usize].len(), 1, "one link in");
        assert_eq!(g.out[dummy as usize].len(), 1, "one link out");
    }
    let flat: usize = g.nodes.iter().map(|n| n.flat_out.len()).sum();
    assert_eq!(flat, 0, "no edge of this graph has its ends on one rank");
}

/// An edge whose ends share a rank is **flat**: it stays in the graph at zero separation and
/// goes into `flat_out`/`flat_in` rather than `out`/`inn`, and gets no dummies.
///
/// `dot`'s own `ED_minlen` is 1, so a plain acyclic graph never produces one — every edge
/// spans at least a rank, and the reverse edge `acyclic` leaves behind gets a chain either
/// way. The branch is reached in practice through a cluster or an explicit `minlen = 0`, so
/// the rank is put side by side here rather than asked for from a fixture.
#[test]
fn an_edge_with_its_ends_on_one_rank_is_flat() {
    let mut g = graph(2, &[(0, 1)]);
    rank(&mut g).expect("connected after cycle breaking");
    assert_eq!(ranks_of(&g), vec![0, 1]);
    g.nodes[1].rank = g.nodes[0].rank;
    class2::run(&mut g);
    assert_eq!(g.nodes.len(), 2, "a flat edge gets no dummies");
    assert_eq!(g.nodes[0].flat_out, vec![0]);
    assert_eq!(g.nodes[1].flat_in, vec![0]);
    assert!(g.out.iter().all(|list| list.is_empty()), "and no chain link");
}

/// A dummy is a `nodesep`-wide placeholder: one point plus `nodesep / 2` on each side
/// (`fastgr.c:200-213` and `class2.c`'s `incr_width`).
#[test]
fn a_chain_dummy_is_a_nodesep_wide_placeholder() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
    rank(&mut g).expect("connected after cycle breaking");
    class2::run(&mut g);
    let dummy = &g.nodes[4];
    assert_eq!(dummy.kind, Kind::Virtual);
    assert_eq!(dummy.lw, 10.0);
    assert_eq!(dummy.rw, 10.0);
    assert_eq!(dummy.ht, 1.0);
}

/// `virtual_weight` (`mincross.c:1706-1731`): a chain link's weight times the table entry for
/// its two endpoint classes. A link between two dummies is weighted **four** — they exist for
/// that edge alone — while a link with a real node at one end keeps its weight. So the middle
/// link of a three-hop chain is four times what the original edge weighed, and that is what
/// makes the simplex care more about a long edge than a short one.
#[test]
fn virtual_weight_scales_a_link_by_its_endpoint_classes() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]);
    rank(&mut g).expect("connected after cycle breaking");
    class2::run(&mut g);
    let low = g.inn[4][0];
    let high = g.out[4][0];
    assert_eq!(g.edges[low as usize].weight, 1, "real node to dummy");
    assert_eq!(g.edges[high as usize].weight, 4, "dummy to dummy");
}

/// Two input edges between the same pair draw as one: the second is folded into the first's
/// chain, so the chain carries both weights and both counts and no second pair of dummies
/// appears.
///
/// `class1` has already given the pair one constraint, so this is `class2`'s own merge: it
/// walks the *input* edges and finds the twin, whose chain is already built. The chain's
/// middle link therefore carries `1 + 1 = 2` on top of the four `virtual_weight` gave it.
#[test]
fn class2_merges_parallel_edges_into_one_chain() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3), (0, 3), (0, 3)]);
    rank(&mut g).expect("connected after cycle breaking");
    let before = g.nodes.len() as u32;
    class2::run(&mut g);
    assert_eq!(
        g.nodes.len() as u32 - before,
        2,
        "one pair of dummies for the pair of parallel edges"
    );
    let middle = g.out[4][0];
    assert_eq!(g.edges[middle as usize].count, 2, "both input edges counted");
    assert_eq!(
        g.edges[middle as usize].weight,
        5,
        "the link's own four, plus the twin's one"
    );
}