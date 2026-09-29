//! The two node sets `layout.bipartite` draws, by SciGraphs' rule
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:149` `_bipartite_parts`,
//! `:181` `_greedy_max_cut`).
//!
//! A graph that two-colours, component by component, is split by its colouring; each
//! component's orientation is the one that keeps the running sets even. Anything else
//! (an odd cycle, a self-loop) falls back to a greedy maximum cut refined by at most
//! [`MAX_CUT_PASSES`] single-vertex passes, so the split is never meaningless and never
//! a refusal. Both walks follow `neighbours` order, which is networkx's.
//!
//! Ponytail: the fallback is a 1/2-approximate cut, not the maximum: on a non-bipartite
//! graph some edges run inside a set and are drawn as vertical lines in one column.
//! Direction: cosmetic. Escape hatch: none needed; a bipartite graph never reaches it.

use std::collections::VecDeque;

/// SciGraphs' `_MAX_CUT_PASSES`.
const MAX_CUT_PASSES: u32 = 8;

const UNCOLOURED: u8 = u8::MAX;

/// `(set0, set1)`, each in the order SciGraphs lists them; together every node once.
pub(super) fn partition(adjacent: &[Vec<u32>]) -> (Vec<u32>, Vec<u32>) {
    two_colour(adjacent).unwrap_or_else(|| greedy_max_cut(adjacent))
}

/// The per-component two-colouring, or `None` at the first odd cycle or self-loop.
fn two_colour(adjacent: &[Vec<u32>]) -> Option<(Vec<u32>, Vec<u32>)> {
    let mut colour = vec![UNCOLOURED; adjacent.len()];
    let (mut set0, mut set1) = (Vec::new(), Vec::new());
    for start in 0..adjacent.len() {
        if colour[start] != UNCOLOURED {
            continue;
        }
        let mut part = colour_component(adjacent, &mut colour, start as u32)?;
        let skew = set0.len() as i64 - set1.len() as i64;
        let (len0, len1) = (part[0].len() as i64, part[1].len() as i64);
        if (skew + len0 - len1).abs() > (skew + len1 - len0).abs() {
            part.swap(0, 1);
        }
        let [first, second] = part;
        set0.extend(first);
        set1.extend(second);
    }
    Some((set0, set1))
}

/// BFS from `start`, colouring alternately; each colour's nodes in discovery order.
fn colour_component(adjacent: &[Vec<u32>], colour: &mut [u8], start: u32) -> Option<[Vec<u32>; 2]> {
    let mut part = [vec![start], Vec::new()];
    colour[start as usize] = 0;
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        let other = 1 - colour[node as usize];
        for &next in &adjacent[node as usize] {
            let seen = colour[next as usize];
            if seen == UNCOLOURED {
                colour[next as usize] = other;
                part[other as usize].push(next);
                queue.push_back(next);
            } else if seen != other {
                return None;
            }
        }
    }
    Some(part)
}

/// Greedy cut then local search, exactly SciGraphs' `_greedy_max_cut`.
fn greedy_max_cut(adjacent: &[Vec<u32>]) -> (Vec<u32>, Vec<u32>) {
    let mut side = vec![UNCOLOURED; adjacent.len()];
    for node in 0..adjacent.len() {
        let placed = side_counts(adjacent, &side, node);
        side[node] = u8::from(placed[0] > placed[1]);
    }
    for _ in 0..MAX_CUT_PASSES {
        let mut moved = false;
        for node in 0..adjacent.len() {
            let around = side_counts(adjacent, &side, node);
            if around[side[node] as usize] > around[1 - side[node] as usize] {
                side[node] = 1 - side[node];
                moved = true;
            }
        }
        if !moved {
            break;
        }
    }
    let of_side = |wanted: u8| {
        (0..side.len() as u32)
            .filter(|&n| side[n as usize] == wanted)
            .collect::<Vec<u32>>()
    };
    (of_side(0), of_side(1))
}

/// How many of `node`'s already-placed neighbours (self excluded) sit on each side.
fn side_counts(adjacent: &[Vec<u32>], side: &[u8], node: usize) -> [u32; 2] {
    let mut counts = [0, 0];
    for &next in &adjacent[node] {
        if next as usize != node && side[next as usize] != UNCOLOURED {
            counts[side[next as usize] as usize] += 1;
        }
    }
    counts
}
