//! The initial order: one breadth-first walk per source, then the transpose pass's say.
//!
//! **Why walk at all.** The rank pass decides which row a node is on and nothing about its
//! left-to-right position. A starting order is still needed, and the reference's is the one
//! that makes a tree come out with no crossings: walk down from every node that has no
//! in-edges, and every node is installed as the walk reaches it, so a node's neighbours are
//! laid out in the order its own edges list them. `acyclic` has already turned the graph into
//! a forest of chains, so this is where the reference's "series-parallel graphs such as trees
//! are drawn with no crossings" comes from.
//!
//! **Both directions, and the better of the two is not chosen.** The walk is run twice over
//! the same ranks: once from the sources, downwards, and once from the sinks, upwards. The
//! second walk overwrites the first, so the second is what the pass starts from — the
//! reference takes the second order and leaves the first to the crossing count's starting
//! point, not to a comparison between the two.
//!
//! **The seeds are in the component's own node order** (which the components pass fixed, not
//! the dense index order), and a node is a seed only if it has no edge *against* the
//! direction being walked: no in-edges for the downward walk, no out-edges for the upward
//! one. A node already reached is not a seed, so the walks partition the component.
//!
//! **The rank is not reversed afterwards.** The reference reverses a rank whose graph is
//! flipped, which is a property of a cluster's internal coordinate system; this port has no
//! clusters, so the rows come out of the walk already in the reference's order.
//!
//! Determinism: the walk is a FIFO queue filled in adjacency-list order over a node list
//! fixed by the components pass; nothing here reads a clock, a hash order or a random number
//! (`prompt.md` §6 D1-D10).

use std::collections::VecDeque;

use super::super::fast::Fast;
use super::ranks::Ranks;

/// Give every node of the component its position, by walking the component from every
/// source (pass 0, downwards) or every sink (pass 1, upwards).
///
/// The walk finishes with one transverse pass over each rank, because a source-only order is
/// not always crossing-free and the reference does not leave a known crossing on the table.
pub fn build_ranks(g: &mut Fast, ranks: &mut Ranks, pass: usize) {
    reset(g, ranks);
    for seed in ranks.nlist().to_vec() {
        if !is_seed(g, seed, pass) || g.nodes[seed as usize].mark {
            continue;
        }
        walk_from(g, ranks, seed, pass);
    }
    for r in 0..=ranks.max() {
        ranks.rows[r].valid = false;
    }
    if super::crossings::ncross(g, ranks) > 0 {
        super::transpose::transpose(g, ranks, false);
    }
}

/// Every node of the component unmarked, and every rank's window emptied. The row past the
/// last rank keeps its spare slot: the transpose pass reads it to find out whether the band
/// below the top rank is empty, and it is.
fn reset(g: &mut Fast, ranks: &mut Ranks) {
    for &node in ranks.nlist() {
        g.nodes[node as usize].mark = false;
    }
    for r in 0..=ranks.max() {
        ranks.rows[r].n = 0;
    }
}

/// A seed is a node with no edge running *against* the walk: no in-edge going down, no
/// out-edge going up.
fn is_seed(g: &Fast, node: u32, pass: usize) -> bool {
    let against = if pass == 0 { &g.inn[node as usize] } else { &g.out[node as usize] };
    against.is_empty()
}

/// One breadth-first walk: install the node reached, queue its unmarked neighbours, and stop
/// when the queue empties.
fn walk_from(g: &mut Fast, ranks: &mut Ranks, seed: u32, pass: usize) {
    let mut queue = VecDeque::new();
    g.nodes[seed as usize].mark = true;
    queue.push_back(seed);
    while let Some(node) = queue.pop_front() {
        ranks.append(g, node);
        enqueue(g, &mut queue, node, pass);
    }
}

/// Queue every unmarked neighbour on the far side of `node`'s edges, in adjacency-list order.
///
/// The two field borrows — the edge list and the mark on the node it leads to — are of
/// different fields of the same graph, which is why the reference can reach both through one
/// pointer and this can too.
fn enqueue(g: &mut Fast, queue: &mut VecDeque<u32>, node: u32, pass: usize) {
    let node = node as usize;
    let edges: &[u32] = if pass == 0 { &g.out[node] } else { &g.inn[node] };
    for &edge in edges {
        let other = {
            let record = &g.edges[edge as usize];
            if pass == 0 {
                record.head
            } else {
                record.tail
            }
        };
        let mark = &mut g.nodes[other as usize].mark;
        if !*mark {
            *mark = true;
            queue.push_back(other);
        }
    }
}