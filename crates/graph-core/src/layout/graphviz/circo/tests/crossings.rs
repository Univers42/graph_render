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
/// out of the middle of what is left, and then — because `longest_path` can carry a node twice
/// and `order_of` does not deduplicate — a draw in four inserts a second copy of a node already
/// placed. One case in four is therefore a **repeated** order, the shape seed 68 of the
/// invariant sweep hands the sweep in anger.
fn order(rng: &mut Draws, count: u32) -> Vec<u32> {
    let mut left: Vec<u32> = (0..count).collect();
    let mut order = Vec::with_capacity(count as usize);
    while !left.is_empty() {
        let at = rng.below(left.len() as u32) as usize;
        order.push(left.remove(at));
    }
    if rng.below(4) == 0 && !order.is_empty() {
        let at = rng.below(order.len() as u32) as usize;
        order.insert(at, order[at]);
    }
    order
}

/// 2 000 random blocks of 2 to 40 nodes, each with a random circle order — one in four of them
/// carrying a node twice, as `longest_path`'s repeated branch does. The sweep and the walk must
/// return the same number on every one.
///
/// A permutation's count is a function of the *cyclic* order alone — no rotation of a circle
/// moves one chord past another — so a random permutation of a fixed graph is not independent
/// coverage, and the closed cases below are what pin the number itself. What the 2 000 cases buy
/// is the sweep's own arithmetic over many shapes and both order kinds: which chord opens and
/// which closes first, what the tree holds between two positions, an edge the order never
/// opens, and the second visit that closes a node's edges a second time.
#[test]
fn the_sweep_agrees_with_the_walk_on_two_thousand_random_blocks() {
    let mut rng = Draws(20_260_403);
    let mut cases = 0;
    let mut repeated = 0;
    let mut crossed = 0u64;
    for _ in 0..2_000 {
        let count = 2 + rng.below(39);
        let edges = edges(&mut rng, count);
        let order = order(&mut rng, count);
        if order.len() > count as usize {
            repeated += 1;
        }
        let (swept, walked) = both_ways(count, &edges, &order);
        assert_eq!(
            swept,
            walked,
            "n={count}, {} edges, swept {swept}, walked {walked}",
            edges.len()
        );
        crossed += u64::from(swept);
        cases += 1;
    }
    assert_eq!(cases, 2_000, "every case ran");
    assert!(repeated > 0, "some orders carried a node twice");
    assert!(
        crossed > 0,
        "the cases crossed something, so this is not a zero-versus-zero pass"
    );
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

/// **A node carried twice**, which is the shape `longest_path` hands the layout when the
/// thinned tree is a forest and its branch node's two best leaves are the same one. The block is
/// `n0 n1`, `n0 n2`, `n1 n3` and the order is `n0, n1, n0, n2, n3`. Worked from the walk, one
/// position at a time:
///
/// - `n0` opens `n0 n1` and `n0 n2` at 1, so nothing closes — 0.
/// - `n1` closes `n0 n1`, and `n0 n2` opened at the same position 1, so not after it — 0. `n1 n3`
///   opens at 2.
/// - **`n0` again.** Both its chords were closed at positions 1 and 1, and the walk closes and
///   counts them a *second* time. `n1 n3` is open, opened at 2 which is after 1, and it touches
///   neither `n1 n2`... it touches `n1`, not `n0`, so the node test cannot see the shared node:
///   each of the two chords counts it, **2**. `n0 n2` then leaves the open set; `n1 n3` stays.
/// - `n2` closes `n0 n2` again and counts `n1 n3`, opened later — **1**.
/// - `n3` closes `n1 n3`, which is all that is left and has nothing after it — 0.
///
/// So **3**, and the two of the three that come from the repeat are the number the port has
/// always produced for such a block. The sweep reproduces them rather than correcting them, which
/// is why this is a closed case and not a comment: it is the difference between the new count and
/// the old one on the input that actually occurs.
#[test]
fn a_node_carried_twice_is_counted_a_second_time() {
    let edges = [(0, 1), (0, 2), (1, 3)];
    assert_eq!(both_ways(4, &edges, &[0, 1, 0, 2, 3]), (3, 3));
}

/// `K2,2` over the parts `{n0, n1}` and `{n2, n3}`, drawn as `n0, n2, n1, n3` and again as
/// `n0, n1, n2, n3`. Its four chords are `n0 n2`, `n0 n3`, `n1 n2`, `n1 n3`, and the two orders
/// differ by how late `n0 n3` is opened relative to `n1 n3`.
///
/// Worked from the walk, first order (`n0` opens `n0 n2` and `n0 n3` at 1, `n2` opens `n1 n2` at
/// 2, `n1` opens `n1 n3` at 3, `n3` closes all three of its row):
///
/// - `n3` closes `n0 n3` (opened at 1) against the still-open `n1 n2` (2) and `n1 n3` (3). The
///   first touches no node of `n3`, **1**; the second shares `n3` and the node test drops it.
/// - `n3` then closes `n1 n3` (3) with nothing opened after it — 0.
///
/// **1**. In the second order `n1` opens `n1 n2` and `n1 n3` both at 2, and `n2` closes `n0 n2`
/// against both of them: `n1 n2` shares `n2` and is dropped, `n1 n3` does not and counts **1**;
/// `n3` then closes `n0 n3` against `n1 n2` for another **1**. **2**. One edge set, two orders,
/// one pair apart, and both are larger than the interleaving count the old sweep reported.
#[test]
fn k22_crosses_one_pair_in_one_order_and_two_in_the_other() {
    let edges = [(0, 2), (0, 3), (1, 2), (1, 3)];
    assert_eq!(both_ways(4, &edges, &[0, 2, 1, 3]), (1, 1));
    assert_eq!(both_ways(4, &edges, &[0, 1, 2, 3]), (2, 2));
}

/// The five-node block of seed 8 of the differential (`docs/measurements/p13-gv1-circo.md` §8),
/// in the circle order its long path reads: `n3, n1, n4, n2, n0`. Not a hand-derived number —
/// this one is what Graphviz's own `count_all_crossings` printed for that order, read off an
/// instrumented 16.1.0 build, and it is **3** where the interleaving count is **0**. Three is
/// what makes `reduce_edge_crossings` fire on this block and land on `[n3, n0, n1, n4, n2]`,
/// which is the circle order Graphviz draws.
///
/// Worked from the walk: `n3` opens `n3 n0` and `n3 n1` at 1; `n1` opens `n1 n0` and `n4 n1` at
/// 2; `n4` opens `n4 n2` at 3; `n2` opens `n2 n0` at 4; `n0` then closes its three.
/// `n0 n0`'s row is `n1 n0` (2), `n2 n0` (4), `n3 n0` (1):
///
/// - closing `n1 n0` (2) against what is open and later: `n4 n2` (3) counts **1** and `n2 n0`
///   (4) shares `n0`, so the node test drops it;
/// - closing `n2 n0` (4) against what is open and later: nothing, 0;
/// - closing `n3 n0` (1): `n4 n1` (2) counts **1** and `n4 n2` (3) counts **1**; `n1 n0` and
///   `n2 n0` share `n0` and are dropped.
///
/// **3**.
#[test]
fn seed8_five_node_block_crosses_three_where_nothing_interleaves() {
    let edges = [(0, 1), (0, 2), (0, 3), (1, 3), (1, 4), (2, 4)];
    assert_eq!(both_ways(5, &edges, &[3, 1, 4, 2, 0]), (3, 3));
}
