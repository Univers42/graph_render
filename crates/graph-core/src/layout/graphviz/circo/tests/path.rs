//! The circle order when the two walks of `longest_path` share a stretch.
//!
//! `longest_path` reads two walks off the thinned tree — one up from the branch node's best leaf,
//! one up from its runner-up — and the block's own nodes are completed by the residual pass. The
//! two walks can share the stretch between the branch node and the point where their leaves
//! diverge, and the node on that stretch is named twice. The reference does the same, so the
//! port keeps the repeat: `docs/measurements/p13-gv1-circo.md` §4c.
//!
//! Every expectation here is checked against `circo -Tplain` in the pinned oracle image; the
//! printed node lines are reproduced in the test's own doc comment.

use super::super::circle;
use super::super::graph::BlockGraph;
use super::super::skeleton;
use super::super::{Block, Derived};
use crate::layout::coords::probe::graph;

/// The order the circle of the one block `edges` induces on `count` nodes carries, as
/// block-local indices: the long path, then the residual pass, then the crossing reduction.
fn circle_order(count: u32, edges: &[(u32, u32)]) -> Vec<u32> {
    let derived = Derived::of(&graph(count, edges));
    let mut spec = Block::empty();
    spec.nodes = (0..count).collect();
    let block = BlockGraph::of(&derived, &spec);
    circle::order_of(&block, skeleton::order_of(&block))
}

/// `order` sorted, so a repeated node and an absent one are the same shape to look at.
fn sorted(order: &[u32]) -> Vec<u32> {
    let mut copy = order.to_vec();
    copy.sort_unstable();
    copy
}

/// **The circle order names `n1` twice, as the reference's does.** The circle's size is the
/// order's own length: `N = LIST_SIZE(&longest_path)` (`blockpath.c:568`) sets the radius
/// (`blockpath.c:571-576`) and every node's slot, `theta = k * 2*PI / N` (`blockpath.c:600`).
/// One node named twice is therefore a ninth slot on an eight-node circle, and the slot the
/// first copy would have taken is left empty.
///
/// The block is a chorded 8-cycle. Its thinned spanning tree puts the branch node `n0` above
/// `n1`, with two leaves under `n1` on either side of it, so `n0`'s two best leaves — `n7` four
/// edges down, `n3` three — are both in `n1`'s subtree and their walks share `n1`. `n4` is the
/// reference's isolated node: it is in no tree at all, so the residual pass is what names it.
///
/// **`circo -Tplain` on this graph** (§4c): eight nodes on a circle of radius `2.50669` in, which
/// is `9 * 1.75 / 2*PI` to five decimals, at `0`, `40`, `80`, `160`, `200`, `240`, `280` and `320`
/// degrees — nine slots, `120` empty, `n1` twice. A port that names each node once draws eight
/// slots and parts from Graphviz on this shape.
#[test]
fn the_circle_order_of_a_chorded_eight_cycle_names_n1_twice_as_the_reference_does() {
    let edges = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 0),
        (0, 3),
        (0, 4),
        (0, 6),
        (1, 4),
        (1, 5),
        (2, 4),
        (4, 7),
        (5, 7),
    ];
    let order = circle_order(8, &edges);
    assert_eq!(
        sorted(&order),
        vec![0, 1, 1, 2, 3, 4, 5, 6, 7],
        "the order names {order:?}: the reference's circle is nine slots, n1 twice"
    );
}
