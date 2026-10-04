//! Scratch probe (temporary, not part of the suite).

use super::super::blocks;
use super::super::circle;
use super::super::graph::{BlockGraph, Derived};
use super::super::skeleton;
use crate::layout::coords::probe::graph;
use crate::layout::graphviz::circo::Layout;

#[test]
fn blocks_in_placement_order() {
    let seed: usize = std::env::var("SEED")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let path = format!(
        "{}/../../../../target/s8/fixture-{seed}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(path).expect("fixture");
    let n: u32 = field(&raw, "\"n\":");
    let source = column(&raw, "source");
    let target = column(&raw, "target");
    let edges: Vec<(u32, u32)> = source
        .iter()
        .zip(target.iter())
        .map(|(a, b)| (*a, *b))
        .collect();
    println!("seed {seed} n={n} edges={}", edges.len());

    let topology = graph(n, &edges);
    let derived = Derived::of(&topology);
    let found = blocks::decompose(&derived, n);
    let layout = Layout::of(found, n);
    for (at, block) in layout.blocks.iter().enumerate() {
        let view = BlockGraph::of(&derived, block);
        let mut order = circle::order_of(&view, skeleton::order_of(&view));
        let realigned = order
            .iter()
            .position(|&local| layout.is_parent(block.nodes[local as usize]));
        if let Some(at) = realigned {
            circle::realign(&mut order, at);
        }
        let named: Vec<u32> = order.iter().map(|&l| block.nodes[l as usize]).collect();
        println!("BLOCK {at} nodes={} circle={named:?}", block.nodes.len());
    }
}

/// The first integer after `key`.
fn field(raw: &str, key: &str) -> u32 {
    raw.split(key)
        .nth(1)
        .unwrap()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

/// Every integer in the array after `key`.
fn column(raw: &str, key: &str) -> Vec<u32> {
    raw.split(key)
        .nth(1)
        .unwrap()
        .split(']')
        .next()
        .unwrap()
        .split(',')
        .filter(|t| !t.trim().is_empty())
        .map(|t| t.trim().parse().unwrap())
        .collect()
}
