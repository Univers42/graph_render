//! Seed 8 of the differential (`docs/measurements/p13-gv1-circo.md` §8): the ten-node fixture
//! whose five-node block the two arms used to draw on the same circle in a different order.
//!
//! The fixture's own edges, in its column order, are what `harness/gv_plain.py`'s `write_dot`
//! turns into `n1 -- n0` twice, `n2 -- n0`, `n3 -- n0` twice, `n4 -- n1` twice, `n5 -- n0`,
//! `n6 -- n0`, `n7 -- n0`, `n8 -- n1`, `n8 -- n0`, `n9 -- n1`, `n9 -- n5`. This test holds the
//! block decomposition, the block's own edge rows, the skeleton's long path and the circle order
//! against the derivation below, so a change in any of them has to be argued for here first.
//!
//! Run with `--nocapture` to read the trace the two arms are compared through.

use super::super::blocks;
use super::super::circle;
use super::super::graph::{BlockGraph, Derived};
use super::super::skeleton;
use crate::layout::coords::probe::graph;
use crate::layout::graphviz::circo::Layout;

/// Seed 8's fixture, `n = 10`, in the column order `circo.jsonl` records it.
const EDGES: [(u32, u32); 14] = [
    (1, 0),
    (1, 0),
    (2, 0),
    (3, 0),
    (3, 0),
    (4, 1),
    (4, 1),
    (5, 0),
    (6, 0),
    (7, 0),
    (8, 1),
    (8, 0),
    (9, 1),
    (9, 5),
];

/// The five-node block `{0, 1, 5, 8, 9}`: the triangle `0-1-8` and the four-cycle `0-1-9-5`
/// share the **edge** `0-1`, which is what makes them one biconnected component rather than
/// two blocks meeting at a cut point. Graphviz's own `-Tplain` puts all five on one circle of
/// radius `5 * 1.75 / 2*PI` inches, at exactly 72 degrees apart.
const BLOCK: [u32; 5] = [0, 1, 5, 8, 9];

#[test]
fn seed8_five_node_block_reads_its_circle_order_off_the_reference_rules() {
    let topology = graph(10, &EDGES);
    let derived = Derived::of(&topology);
    let found = blocks::decompose(&derived, 10);
    let layout = Layout::of(found, 10);

    let Some(at) = layout.blocks.iter().position(|block| block.nodes == BLOCK) else {
        for (at, block) in layout.blocks.iter().enumerate() {
            println!("block {at}: nodes {:?}", block.nodes);
        }
        panic!("no five-node block {{0,1,5,8,9}}");
    };
    let block = &layout.blocks[at];
    let view = BlockGraph::of(&derived, block);
    let named = |local: &[u32]| -> Vec<u32> {
        local.iter().map(|&at| block.nodes[at as usize]).collect()
    };

    let path = skeleton::order_of(&view);
    let mut order = circle::order_of(&view, path.clone());
    let realigned = order
        .iter()
        .position(|&local| layout.is_parent(block.nodes[local as usize]));
    if let Some(at) = realigned {
        circle::realign(&mut order, at);
    }

    println!("five-node block at {at}, block-local nodes {:?}", block.nodes);
    for node in 0..10u32 {
        println!("  n{node} derives to {:?}", derived.neighbours(node));
    }
    println!("  block edges, block-local (tail, head): {:?}", view.ends());
    for local in 0..view.nodes.len() as u32 {
        let row: Vec<(u32, u32)> = view
            .row(local)
            .iter()
            .map(|&edge| (view.other(edge, local), edge))
            .collect();
        println!("  n{} row {row:?}", block.nodes[local as usize]);
    }
    println!("  skeleton long path (local) {path:?} -> {:?}", named(&path));
    println!("  circle order (local) {order:?} -> {:?}", named(&order));
    println!("  realigned at {realigned:?}");

    // `remove_pair_edges` over this block, from `blockpath.c:182-222`. Block-local degrees are
    // `[3, 3, 2, 2, 2]`, so `n9` is at the back of the degree-descending list and `n5` is at the
    // back after the first pass — the pass that links `n1 -- n5` to put `n9`'s degree back up.
    // The second pass finds `n1 -- n0` to be the pair edge and deletes it from `outg`, leaving the
    // five-cycle `0-5-9-1-8-0`. The long path is read off that cycle's spanning tree from the leaf
    // `n8`, because `n0` is the tree root and `measure_distance` returns at once on a node with
    // no parent.
    assert_eq!(named(&path), vec![8, 1, 9, 5, 0], "the long path");
    // That order has **no** interleaving pair of chords, so a count that retired each edge when
    // it closed would leave `reduce_edge_crossings` with nothing to do. The reference's own
    // `remove_edge` never retires anything (see `circo/crossings.rs`), so the count is 3 and the
    // reduction moves `n0` in front of `n1` — `insertNodelist(&list, n0, n1, 1)`, the second of
    // the two slots, the first try having moved it in front and been refused. `n1` carries
    // `PARENT_F`, so the order is then rotated left by one and `n0` lands on slot 0.
    //
    // The last line is the point of this test: reading Graphviz's own angles off its `-Tplain`
    // gives exactly this order, `n0` at 0 degrees and the rest 72 apart.
    assert_eq!(named(&order), vec![0, 1, 9, 5, 8], "the circle order");
}
