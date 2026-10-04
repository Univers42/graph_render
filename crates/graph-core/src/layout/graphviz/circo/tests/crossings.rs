//! The crossing count from both sides: 2 000 random blocks where the Fenwick sweep and the
//! reference's walk have to return the same number, and five closed cases worked out by hand.

use super::super::crossings::{Counter, count_all_crossings};
use super::super::graph::BlockGraph;
use super::super::{Block, Derived};
use crate::layout::coords::probe::graph;

/// The block `edges` induces on `count` nodes, with `order` as its circle order, counted by the
/// sweep and by the reference's walk: `(swept, walked)`.
fn both_ways(count: u32, edges: &[(u32, u32)], order: &[u32]) -> (u32, u32) {
    let derived = Derived::of(&graph(count, edges));
    let mut spec = Block::empty();
    spec.nodes = (0..count).collect();
    let mut block = BlockGraph::of(&derived, &spec);
    let swept = Counter::default().count(&block, order);
    (swept, count_all_crossings(&mut block, order))
}

#[test]
fn tmp_order_lengths() {
    use crate::index::{index_model, seeded_model};
    use crate::registry::REFERENCE_DEGREE;
    for seed in 0..200u32 {
        let (nodes, edges) = seeded_model(seed, 2 + seed % 600, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("indexes");
        let count = topology.node_count();
        let derived = Derived::of(&topology);
        let found = super::super::blocks::decompose(&derived, count);
        let layout = super::super::Layout::of(found, count);
        for at in 0..layout.blocks.len() {
            let order = layout.circle_of(&derived, at);
            let spec = &layout.blocks[at].nodes;
            if order.len() != spec.len() {
                let mut sorted = order.clone();
                sorted.sort_unstable();
                let mut uniq = sorted.clone();
                uniq.dedup();
                panic!(
                    "seed {seed} block {at}: {} nodes, order len {}, distinct {} of {}",
                    spec.len(),
                    order.len(),
                    uniq.len(),
                    order.len()
                );
            }
        }
    }
}

/// A fixed-seed 64-bit LCG: the multiplier `6 364 136 223 846 793 005` and the increment
/// `1 442 695 040 888 963 407` of Numerical Recipes' `ranqd1`, modulo `2^64` by `wrapping_*`,
/// with the top 32 bits as the draw. Integer arithmetic only, no clock, so the case list is the
/// same list on every host and on wasm32 (`prompt.md` §6 D3, D4).
struct Draws(u64);

impl Draws {
    /// The next draw.
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 32) as u32
    }

    /// A draw below `bound`, which is never 0 at any call site here.
    fn below(&mut self, bound: u32) -> u32 {
        self.next() % bound
    }
}

/// A random simple graph on `count` nodes: every unordered pair is an edge with probability
/// `35/100`, so a case runs from no edges at all to a near-clique. No pair is repeated and no
/// node is its own neighbour.
fn edges(rng: &mut Draws, count: u32) -> Vec<(u32, u32)> {
    let mut edges = Vec::new();
    for one in 0..count {
        for other in one + 1..count {
            if rng.below(100) < 35 {
                edges.push((one, other));
            }
        }
    }
    edges
}

/// A random circle order of `count` nodes: the identity walked once, each draw taking one node
/// out of the middle of what is left.
fn order(rng: &mut Draws, count: u32) -> Vec<u32> {
    let mut left: Vec<u32> = (0..count).collect();
    let mut order = Vec::with_capacity(count as usize);
    while !left.is_empty() {
        let at = rng.below(left.len() as u32) as usize;
        order.push(left.remove(at));
    }
    order
}

/// 2 000 random blocks of 2 to 40 nodes, each with a random circle order: the sweep and the
/// walk must return the same number on every one of them.
///
/// The count is a function of the *cyclic* order alone — no rotation of a circle moves one
/// chord past another — so a random order of a fixed graph is not independent coverage, and
/// that is what the closed cases below are for. What the 2 000 cases do buy is the sweep's own
/// index arithmetic over many shapes: which chord closes first, what the tree holds when it
/// does, which pairs share a node, and how the counters carry.
#[test]
fn the_sweep_agrees_with_the_walk_on_two_thousand_random_blocks() {
    let mut rng = Draws(20_260_403);
    let mut cases = 0;
    let mut crossed = 0u64;
    for _ in 0..2_000 {
        let count = 2 + rng.below(39);
        let edges = edges(&mut rng, count);
        let order = order(&mut rng, count);
        let (swept, walked) = both_ways(count, &edges, &order);
        assert_eq!(
            swept, walked,
            "n={count}, {} edges, swept {swept}, walked {walked}",
            edges.len()
        );
        crossed += u64::from(swept);
        cases += 1;
    }
    assert_eq!(cases, 2_000, "every case ran");
    assert!(crossed > 0, "the cases crossed something, so this is not a zero-versus-zero pass");
}

/// `K4` in the order `n0, n1, n2, n3`: six chords, of which `n0 n1`, `n0 n3` and `n2 n3` run
/// along the circle's own rim and `n1 n2` sits inside them, so the only pair whose four
/// endpoints interleave is `n0 n2` with `n1 n3` — one crossing.
#[test]
fn a_complete_four_in_its_own_order_crosses_exactly_once() {
    let edges = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    assert_eq!(both_ways(4, &edges, &[0, 1, 2, 3]), (1, 1));
}

/// A triangle: `n0 n1`, `n1 n2` and `n0 n2`. Every pair of the three chords shares a node, and a
/// shared node is never a crossing, so the count is zero.
#[test]
fn a_triangle_crosses_nothing() {
    let edges = [(0, 1), (1, 2), (0, 2)];
    assert_eq!(both_ways(3, &edges, &[0, 1, 2]), (0, 0));
}

/// Two chords on four nodes that share nothing: `n0 n1` and `n2 n3`, a pair of parallel chords
/// of the circle. Zero.
#[test]
fn two_parallel_chords_cross_nothing() {
    let edges = [(0, 1), (2, 3)];
    assert_eq!(both_ways(4, &edges, &[0, 1, 2, 3]), (0, 0));
}

/// `K2,2` over the parts `{n0, n1}` and `{n2, n3}`, drawn as `n0, n2, n1, n3`. Its four chords
/// are `n0 n2`, `n0 n3`, `n1 n2`, `n1 n3`; four of the six pairs share a node, and `n0 n3`
/// lies wholly around `n1 n2`, so nothing crosses. Drawn as `n0, n1, n2, n3` the same four edges
/// are `n0 n2`, `n0 n3`, `n1 n2`, `n1 n3` in position terms and `n0 n2` now interleaves with
/// `n1 n3` — one crossing. One edge set, two orders, one pair apart.
#[test]
fn k22_crosses_one_pair_in_one_order_and_none_in_the_other() {
    let edges = [(0, 2), (0, 3), (1, 2), (1, 3)];
    assert_eq!(both_ways(4, &edges, &[0, 2, 1, 3]), (0, 0));
    assert_eq!(both_ways(4, &edges, &[0, 1, 2, 3]), (1, 1));
}