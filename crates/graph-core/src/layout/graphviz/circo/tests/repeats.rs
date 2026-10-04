//! TEMPORARY: how many of the 1000 gate seeds carry a block whose circle order repeats a node.

use super::super::graph::Derived;
use super::super::{Layout, blocks};
use crate::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};

#[test]
fn count_the_repeats_over_the_gate_seeds() {
    let mut repeats = 0usize;
    let mut worst = 0i64;
    let mut seeds: Vec<u32> = Vec::new();
    for seed in 0..1000u32 {
        let count = gate_node_count(seed);
        let (nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
        let topology = index_model(&nodes, &edges).expect("indexes");
        let derived = Derived::of(&topology);
        let mut layout = Layout::of(blocks::decompose(&derived, count), count);
        let mut found = 0usize;
        for at in 0..layout.blocks.len() {
            let order = layout.circle_of(&derived, at);
            let mut sorted = order.clone();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() != order.len() {
                found += 1;
            }
        }
        if found > 0 {
            repeats += 1;
            worst = worst.max(found as i64);
            seeds.push(seed);
        }
    }
    println!("seeds carrying a repeat: {repeats} of 1000, worst {worst} blocks in one seed");
    println!("first seeds: {:?}", &seeds[..seeds.len().min(40)]);
    println!("seed 68 present: {}", seeds.contains(&68));
}