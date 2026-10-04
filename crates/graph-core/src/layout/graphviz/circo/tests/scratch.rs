//! Scratch probe (temporary).

use super::super::blocks;
use super::super::circle;
use super::super::graph::{BlockGraph, Derived};
use super::super::skeleton;
use crate::layout::coords::probe::{graph, points};
use crate::layout::graphviz::circo::Layout;

const CASES: &[(&str, u32, &[(u32, u32)])] = &[
    ("b5", 5, &[(1, 0), (2, 0), (3, 1), (3, 0), (4, 1), (4, 2)]),
    ("c4", 4, &[(1, 0), (2, 0), (3, 1), (3, 2)]),
    ("b5-no31", 5, &[(1, 0), (2, 0), (3, 0), (4, 1), (4, 2)]),
    ("b5-no41", 5, &[(1, 0), (2, 0), (3, 1), (3, 0), (4, 2)]),
    ("b5-no20", 5, &[(1, 0), (3, 1), (3, 0), (4, 1), (4, 2)]),
    ("b5-rev", 5, &[(0, 1), (0, 2), (1, 3), (0, 3), (1, 4), (2, 4)]),
];

#[test]
fn probe() {
    for (name, count, edges) in CASES {
        let topology = graph(*count, edges);
        let derived = Derived::of(&topology);
        let found = blocks::decompose(&derived, *count);
        let layout = Layout::of(found, *count);
        println!("{name}: blocks");
        for (at, block) in layout.blocks.iter().enumerate() {
            let view = BlockGraph::of(&derived, block);
            let path = skeleton::order_of(&view);
            let mut order = circle::order_of(&view, path.clone());
            let at2 = order
                .iter()
                .position(|&local| layout.is_parent(block.nodes[local as usize]));
            if let Some(at2) = at2 {
                circle::realign(&mut order, at2);
            }
            let named = |local: &[u32]| -> Vec<u32> {
                local.iter().map(|&a| block.nodes[a as usize]).collect()
            };
            println!(
                "  block {at} nodes {:?} path {:?} circle {:?}",
                block.nodes,
                named(&path),
                named(&order)
            );
        }
    }
}
