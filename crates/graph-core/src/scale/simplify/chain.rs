//! Chain contraction: maximal degree-2 runs become one journalled representative-level edge.

use super::*;

/// Maximal runs of degree-2 nodes between two branch nodes (degree ≠ 2) contract to one
/// representative-level edge. A run that closes on itself is a bare ring: there is no
/// second endpoint to imply an edge to, so it is left alone rather than half-drawn.
///
/// Every run is walked once: its nodes are marked seen whether or not it contracts, so a
/// ring or a skipped run is not re-walked from each of its nodes (review R23).
///
/// Ponytail: a run that ends on a leaf the leaf pass folded is left whole (review R7).
/// Failing input: a pendant path, branch node to leaf. Direction: under-reduction, its
/// interior stays drawn; never a link to a hidden node.
pub(super) fn contract_chains(t: &Topology, graph: &Simple, out: &mut Simplified) {
    let mut seen = vec![false; t.node_count() as usize];
    for start in 0..t.node_count() {
        if graph.degree(start) != 2 || out.visible[start as usize] == 0 || seen[start as usize] {
            continue;
        }
        let (lo, hi, path) = walk_chain(graph, start);
        for &node in &path {
            seen[node as usize] = true;
        }
        if lo == hi || out.visible[lo as usize] == 0 || out.visible[hi as usize] == 0 {
            continue;
        }
        for &node in &path {
            out.visible[node as usize] = 0;
            out.representative[node as usize] = lo.min(hi);
        }
        let edges = chain_edges(t, &path, lo, hi);
        for &edge in &edges {
            out.edges[edge as usize] = 0;
        }
        out.steps.push(Step {
            kind: Kind::Chain,
            representative: lo.min(hi),
            nodes: {
                let mut ascending = path.clone();
                ascending.sort_unstable();
                ascending
            },
            edges,
            links: vec![(lo.min(hi), lo.max(hi))],
        });
    }
}

/// One maximal degree-2 run through `start`, both ways: the branch nodes at its ends
/// (or `start` itself, twice, when the run closes on itself) and the interior nodes in
/// ascending order. The two directions are taken in the row's own order — ascending, so
/// `lo` is the walk that ends at the lower branch node — and nothing here depends on
/// which direction ran first.
fn walk_chain(graph: &Simple, start: u32) -> (u32, u32, Vec<u32>) {
    let row = graph.row(start);
    let (first, second) = (row[0], row[1]);
    let mut to_lo = Vec::new();
    let lo = walk_chain_end(graph, start, first, &mut to_lo);
    let mut to_hi = Vec::new();
    let hi = walk_chain_end(graph, start, second, &mut to_hi);
    // Path order from `lo` to `hi`, which is what names the chain's own edges: the two
    // walks run outward from `start`, so the `lo` side is reversed and `start` joins them.
    to_lo.reverse();
    to_lo.push(start);
    to_lo.extend(to_hi);
    (lo, hi, to_lo)
}

/// The branch node one end of a degree-2 run reaches, recording the nodes it walked
/// through. A degree-2 graph is a disjoint union of paths and rings, so this terminates:
/// a run either meets a node of another degree or comes back to where it started.
fn walk_chain_end(graph: &Simple, from: u32, first_step: u32, interior: &mut Vec<u32>) -> u32 {
    let mut previous = from;
    let mut current = first_step;
    loop {
        if current == from {
            return from;
        }
        if graph.degree(current) != 2 {
            return current;
        }
        interior.push(current);
        match graph.row(current).iter().copied().find(|&n| n != previous) {
            Some(next) => {
                previous = current;
                current = next;
            }
            None => return from,
        }
    }
}

/// The chain's own edges: the path `lo, path…, hi`, hop by hop, plus every interior
/// node's self-loops, ascending and deduplicated. A parallel edge on one hop is removed
/// with it — the hop is gone, so every copy of it is. Each hop is read from its interior
/// end, so the cost is the interior nodes' degrees; the hops are distinct pairs because
/// `lo != hi` and the path never repeats a node.
fn chain_edges(t: &Topology, path: &[u32], lo: u32, hi: u32) -> Vec<u32> {
    let (first, last) = (path[0], path[path.len() - 1]);
    let mut edges = edges_between(t, first, lo);
    for hop in path.windows(2) {
        edges.extend(edges_between(t, hop[0], hop[1]));
    }
    edges.extend(edges_between(t, last, hi));
    for &node in path
        .get(1..path.len().saturating_sub(1))
        .unwrap_or_default()
    {
        edges.extend(self_loops(t, node));
    }
    edges.sort_unstable();
    edges.dedup();
    edges
}
