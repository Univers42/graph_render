//! Seed 8 of the differential (`docs/measurements/p13-gv1-circo.md` §5): the ten-node fixture
//! whose five-node block the two arms draw on the same circle in a different order.
//!
//! The fixture's own edges, in its column order, are what `harness/gv_plain.py`'s `write_dot`
//! turns into `n1 -- n0` twice, `n2 -- n0`, `n3 -- n0` twice, `n4 -- n1` twice, `n5 -- n0`,
//! `n6 -- n0`, `n7 -- n0`, `n8 -- n1`, `n8 -- n0`, `n9 -- n1`, `n9 -- n5`. This test holds the
//! block decomposition, the block's own edge rows, the skeleton's long path and the circle
//! order against a hand derivation from `blockpath.c`, so a change in any of them has to be
//! argued for here first.
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
fn seed8_five_node_block_is_one_circle_read_from_the_reference_rules() {
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

    // The hand derivation of `remove_pair_edges` over this block, from `blockpath.c:182`:
    // degrees `[3, 3, 2, 2, 2]` in block order leave `n9` at the back of the degree-descending
    // list, and the first pass links `n1 -- n5` to put `n9`'s degree back up; `n5` is then at
    // the back. The second pass finds `n1 -- n0` to be the pair edge and deletes it from `outg`,
    // leaving the five-cycle `0-5-9-1-8-0`, whose long path is read from the leaf `n8` because
    // `n0` is the tree root and its own `measure_distance` call returns at once.
    assert_eq!(named(&path), vec![8, 1, 9, 5, 0], "the long path");
    // Graphviz's own `-Tplain` angles put `n0`, `n1`, `n9`, `n5`, `n8` on slots 0..4, i.e.
    // the cycle `0-1-9-5-8`, which is **not** the cycle `8-1-9-5-0` the long path reads. Five
    // nodes with no crossings, so `reduce_edge_crossings` returns the list untouched
    // (`blockpath.c:477-484`); the two orders therefore differ before that, and this
    // assertion is what says so.
    assert_eq!(named(&order), vec![1, 9, 5, 0, 8], "the circle order");
}
