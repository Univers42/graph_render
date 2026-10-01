//! The reference's BFS half, verbatim: `_bfs_levels` (`hierarchical.py:12-29`),
//! `_component_roots` (`:31-52`), `_multi_source_levels` (`:54-81`) and `_group_by_level`
//! (`:83-88`). Nothing here does arithmetic on coordinates, only integer depths in dense
//! index order.

use crate::layout::force::SimpleGraph;

/// A node no sweep has reached. `u32::MAX` rather than an `Option`, so a depth column is
/// one `u32` per node and "levelled" is one comparison.
const UNLEVELED: u32 = u32::MAX;

/// `_component_roots` (`hierarchical.py:31-52`): one root per connected component, the
/// midpoint of the double-BFS diameter — exact on trees, where the maximum-degree node is
/// usually not the root.
///
/// Components are taken in ascending dense index, which is the reference's `G.nodes()`
/// order, and each sweep walks `SimpleGraph::rows` in simple-edge order, which is the
/// insertion order `nx.Graph.neighbors` yields.
pub(crate) fn component_roots(graph: &SimpleGraph, n: u32) -> Vec<u32> {
    let mut seen = vec![false; n as usize];
    let mut depths = vec![UNLEVELED; n as usize];
    let mut parents = vec![UNLEVELED; n as usize];
    let mut roots = Vec::new();
    for start in 0..n {
        if seen[start as usize] {
            continue;
        }
        let first = sweep(graph, start, &mut depths, &mut parents);
        for &node in &first {
            seen[node as usize] = true;
        }
        // `order[-1]` is the last node the first sweep reached, the reference's farthest
        // guess; the second sweep starts there and its own last node is the far end.
        let far = sweep(
            graph,
            *first.last().expect("a component holds its start"),
            &mut depths,
            &mut parents,
        );
        let mut path = vec![*far.last().expect("a component holds its start")];
        while parents[path[path.len() - 1] as usize] != UNLEVELED {
            path.push(parents[path[path.len() - 1] as usize]);
        }
        roots.push(path[path.len() / 2]);
    }
    roots
}

/// One `_bfs_levels` sweep (`hierarchical.py:16-29`) from `start`: the visit order, with
/// `depths` and `parents` filled for the nodes it reached and reset to [`UNLEVELED`]
/// before it returns, so the next component starts clean without an `O(n)` clear each.
fn sweep(graph: &SimpleGraph, start: u32, depths: &mut [u32], parents: &mut [u32]) -> Vec<u32> {
    let mut order = vec![start];
    depths[start as usize] = 0;
    parents[start as usize] = UNLEVELED;
    let mut head = 0;
    while head < order.len() {
        let node = order[head];
        head += 1;
        let depth = depths[node as usize] + 1;
        for edge in graph.rows.row(node) {
            let next = graph.other(*edge, node);
            if depths[next as usize] == UNLEVELED {
                depths[next as usize] = depth;
                parents[next as usize] = node;
                order.push(next);
            }
        }
    }
    for &node in &order {
        depths[node as usize] = UNLEVELED;
    }
    order
}

/// `_multi_source_levels` (`hierarchical.py:54-81`): BFS depth from every root at once,
/// and a node no root reaches seeds a level zero of its own so that every node ends up
/// with a level. Returns `(node, level)` in BFS discovery order, which is the order the
/// reference's `levels` dict is inserted in and therefore the order `_group_by_level`
/// groups by.
///
/// Its `for node in pending:` loop iterates `G.nodes()`; this walks dense index `0..n`,
/// the same sequence.
pub(crate) fn multi_source_levels(graph: &SimpleGraph, roots: &[u32], n: u32) -> Vec<(u32, u32)> {
    let mut depths = vec![UNLEVELED; n as usize];
    let mut queue: Vec<u32> = Vec::with_capacity(n as usize);
    for &root in roots {
        if depths[root as usize] == UNLEVELED {
            depths[root as usize] = 0;
            queue.push(root);
        }
    }
    let mut head = 0;
    let mut pending = 0u32;
    loop {
        head = drain(graph, &mut queue, &mut depths, head);
        let Some((node, next)) = reseed(&depths, pending) else {
            break;
        };
        pending = next;
        depths[node as usize] = 0;
        queue.push(node);
    }
    queue
        .iter()
        .map(|&node| (node, depths[node as usize]))
        .collect()
}

/// The reference's inner `while head < len(queue)` sweep, one round of it: every node the
/// frontier holds is levelled one deeper and enqueued. Returns the new head, which is the
/// queue's length once the frontier is drained — the queue is never popped from, so the
/// insertion order the depths are read back in is the discovery order (D3).
fn drain(graph: &SimpleGraph, queue: &mut Vec<u32>, depths: &mut [u32], mut head: usize) -> usize {
    while head < queue.len() {
        let node = queue[head];
        head += 1;
        let depth = depths[node as usize] + 1;
        for edge in graph.rows.row(node) {
            let next = graph.other(*edge, node);
            if depths[next as usize] == UNLEVELED {
                depths[next as usize] = depth;
                queue.push(next);
            }
        }
    }
    head
}

/// The reference's `for node in pending: ... break / else: return` (`hierarchical.py:75-80`),
/// as a cursor over dense index: the lowest-index node no sweep has reached, and the cursor
/// past it. `None` is the `else: return`, so a re-seed never re-scans what it already passed.
fn reseed(depths: &[u32], cursor: u32) -> Option<(u32, u32)> {
    let mut node = cursor;
    while node < depths.len() as u32 {
        if depths[node as usize] == UNLEVELED {
            return Some((node, node + 1));
        }
        node += 1;
    }
    None
}

/// `_group_by_level` (`hierarchical.py:83-88`) as a level-major table, one bucket per
/// level `0..=max_level` and each bucket in BFS discovery order (D2/D3 — see the module
/// doc on why that is the reference's dict order and not a substitute for it).
pub(crate) fn by_level(depths: &[(u32, u32)]) -> Vec<Vec<u32>> {
    let width = depths
        .iter()
        .map(|&(_, level)| level + 1)
        .max()
        .unwrap_or(0);
    let mut rings: Vec<Vec<u32>> = vec![Vec::new(); width as usize];
    for &(node, level) in depths {
        rings[level as usize].push(node);
    }
    rings
}
