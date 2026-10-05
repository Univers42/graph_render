//! The `pair_up` fan branch: the smallest block on which `DEGREE` decides the spanning tree
//! (`docs/measurements/p13-gv1-circo.md` §9), and the 74-node block of seed 100 that the same
//! divergence was first seen on.
//!
//! `find_pair_edges` classifies `node`'s neighbours and hands `pair_up` a degree top-up of
//! `node_degree - 1 - edge_cnt` (`blockpath.c:139`). When that equals the number of unpaired
//! neighbours, the reference fans them all off **one** hub — the first paired neighbour — and
//! raises `DEGREE` on the hub and on each fanned neighbour **once** (`blockpath.c:161-172`).
//!
//! [`EDGES`] is the smallest graph where that shows. `remove_pair_edges` runs
//! `nodeCount - 3` = 3 rounds on it, one of which fans a neighbour off the hub, and the
//! reference's own instrumented trace gives `DEGREE` on the fanned neighbour as **1**
//! afterwards. This port used to leave it at **2**, because `Work::link` raises both endpoints
//! and the fan branch raised the fanned endpoint a second time by hand. `DEGREE` is what the
//! next round's `LIST_SORT` orders on, so one extra point moved that node across its hub in the
//! degree list, `remove_pair_edges` took the two in the opposite order, and the two spanning
//! trees were made of different edges.
//!
//! `-Tplain` on [`EDGES`] puts `n2` at `3.7173 1.6972`, `n4` at `2.8817 3.1445`, `n3` at
//! `1.2106 3.1445`, `n1` at `0.375 1.6972`, `n5` at `1.2106 0.25` and `n0` at `2.8817 0.25` —
//! slots 0 to 5 of this port's own circle, in that order.

use super::super::blocks;
use super::super::graph::{BlockGraph, Derived};
use super::super::skeleton;
use super::run;
use super::slot;
use crate::layout::coords::probe::{assert_close, graph, points};
use crate::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};

/// The six-node block, in the column order the fixture carries it: `n3` is the hub's other
/// neighbour and `n5` the one the fan branch hangs off it.
const EDGES: [(u32, u32); 10] = [
    (0, 1),
    (0, 3),
    (0, 5),
    (1, 3),
    (1, 4),
    (1, 5),
    (2, 3),
    (2, 4),
    (2, 5),
    (3, 4),
];

/// All six nodes are one biconnected component, so there is one block and no cut point.
const BLOCK: [u32; 6] = [0, 1, 2, 3, 4, 5];

/// The reference's own `TRACE longest_path` for [`EDGES`], and the port's before the fix.
const SIX_PATH: [u32; 6] = [3, 2, 4, 1, 5, 0];
const SIX_PATH_BEFORE: [u32; 5] = [2, 4, 1, 5, 0];

#[test]
fn the_skeleton_fans_a_degree_top_up_off_one_hub_without_double_counting() {
    let topology = graph(6, &EDGES);
    let derived = Derived::of(&topology);
    let found = blocks::decompose(&derived, 6);
    let layout = super::super::Layout::of(found, 6);
    assert_eq!(layout.blocks.len(), 1, "one block, no cut point");
    assert_eq!(layout.blocks[0].nodes, BLOCK);

    let view = BlockGraph::of(&derived, &layout.blocks[0]);
    let path = skeleton::order_of(&view);
    let named: Vec<u32> = path
        .iter()
        .map(|&local| layout.blocks[0].nodes[local as usize])
        .collect();
    assert_eq!(named, SIX_PATH, "the long path");
    assert_ne!(named.as_slice(), SIX_PATH_BEFORE, "the path before the fix");

    // `-Tplain` puts `n2` at `3.7173 1.6972`, `n4` at `2.8817 3.1445`, `n3` at
    // `1.2106 3.1445`, `n1` at `0.375 1.6972`, `n5` at `1.2106 0.25` and `n0` at
    // `2.8817 0.25` — slots 0 to 5 of this port's own circle, in that order.
    let got = points(&run(&topology).expect("lays out"));
    assert_close(
        &got,
        &[
            slot(6.0, 5.0),
            slot(6.0, 3.0),
            slot(6.0, 0.0),
            slot(6.0, 2.0),
            slot(6.0, 1.0),
            slot(6.0, 4.0),
        ],
        1e-2,
    );
}

/// Seed 100 of the differential (`n = 102`) and its one block of 74 nodes: the block §8.6
/// stopped on. Run with `--nocapture` to read this port's trace of all 71 rounds of
/// `remove_pair_edges`, printed in the instrumented reference's own format so the two diff
/// line for line. Every round agrees with it; §9 has what the comparison showed.
#[test]
fn seed100_thirtyfour_node_block_thins_to_the_reference_skeleton() {
    let seed = 100;
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    let derived = Derived::of(&topology);
    let found = blocks::decompose(&derived, n);
    let layout = super::super::Layout::of(found, n);
    let Some(at) = layout.blocks.iter().position(|b| b.nodes.len() == 74) else {
        panic!("no 74-node block");
    };
    let block = &layout.blocks[at];
    let view = BlockGraph::of(&derived, block);
    skeleton::enable_trace(&block.nodes);
    let path = skeleton::order_of(&view);
    let named: Vec<u32> = path
        .iter()
        .map(|&local| block.nodes[local as usize])
        .collect();
    println!("longest_path {named:?}");

    // The instrumented reference's own line for this block, `TRACE longest_path`. Before the
    // fix this port read `[30 81 37 63 86 4 26 5 20 16 21 1 48 99 6 24 3 8 49 64 15 11 36
    // 89 27 92 94 12 2 29 9 7 0]`: the same `DISTTWO` half, the same branch node `n0`, and a
    // `LEAFONE` walk up the middle chain the other way.
    assert_eq!(
        named,
        vec![
            87, 38, 17, 28, 95, 71, 48, 1, 21, 16, 20, 5, 26, 4, 86, 63, 37, 6, 24, 3, 8, 49, 64,
            15, 11, 36, 89, 27, 92, 94, 12, 2, 29, 9, 7, 0
        ],
        "the reference's long path"
    );
}
