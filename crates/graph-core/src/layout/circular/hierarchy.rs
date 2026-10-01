//! `layout.circular.hierarchy`: SciGraphs' own `CIRCULAR_HIERARCHY`
//! (`_circular_hierarchy_layout`,
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:693-732`), ported whole:
//! one concentric ring per BFS level, level 0 on the axis, `Point` nodes, straight `Line`
//! edges, `O(n + m)`, no RNG and no iteration.
//!
//! **The third ring layout, and the only one of the three that is SciGraphs' function.**
//! `layout.circular.radial` (`super`) is a radial over the motor's *repaired* hierarchy
//! (`super::super::hierarchy::Hierarchy`, `docs/decisions/circular-conventions.md`), with
//! radius linear in the ring number; this one is SciGraphs' own closed form, radius
//! `max(level, 0.35) * scale / max(2, max_level)`; `layout.circular.ring` (`super::ring`)
//! ignores the graph and puts every node on one circle. Pick by the question asked:
//! repaired tree, SciGraphs' levels, or no structure at all.
//!
//! **A directed input is read undirected, and that is the port's one stated departure.**
//! SciGraphs branches on `G.is_directed()`: a digraph's roots are its in-degree-0 sources,
//! with a no-source fallback to its three biggest fan-outs (`hierarchical.py:705-711`), and
//! `neighbors` then means successors. The motor's [`Topology`] has no whole-graph
//! directedness — `edges().directed` is per edge (`crates/graph-core/src/columns.rs:118`)
//! and every layout here reads the undirected projection
//! ([`crate::layout::force::SimpleGraph`]) — so this port takes the undirected branch
//! (`_component_roots`, `hierarchical.py:31-52`) and the differential's SciGraphs arm
//! builds an `nx.Graph` to match it.
//!
//! The node set is the undirected, deduplicated, self-loop-free one every force layout
//! already shares ([`crate::layout::force::simple_graph`]), so the SciGraphs arm sees one
//! graph too: `nx.Graph` merges a repeated unordered pair silently and its `neighbors`
//! yields insertion order, which is the CSR row order this module walks.

use super::super::Geometry;
use super::super::coords::point_geometry;
use crate::index::Topology;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::stage::StageError;

/// The layout's capability id, which is also its hash-gate stage.
///
/// Not a `Stage::ID`: this module takes no `Params` — `ITERATIONS` does not exist here and
/// `scale` is SciGraphs' own dispatcher default, so the conventions are pinned by this
/// module the way `treemap.rs` and `tidy_tree.rs` pin theirs.
pub const ID: &str = "layout.circular.hierarchy";

/// SciGraphs' `apply_graph_layout(scale=5.0)` default (`dispatcher.py:14`), the value the
/// dispatcher hands `_circular_hierarchy_layout`.
const SCALE: f64 = 5.0;

/// A node no sweep has reached. `u32::MAX` rather than an `Option`, so a level column is
/// one `u32` per node and "levelled" is one comparison.
const UNLEVELED: u32 = u32::MAX;

/// Runs the layout over `topology`'s undirected projection; never refuses, like the
/// reference it ports: every branch of it is total.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let graph = simple_graph(topology);
    let n = topology.node_count();
    let roots = component_roots(&graph, n);
    let levels = multi_source_levels(&graph, &roots, n);
    let (x, y) = points(&levels, n);
    Ok(point_geometry(&x, &y))
}

/// `_component_roots` (`hierarchical.py:31-52`): one root per connected component, the
/// midpoint of the double-BFS diameter — exact on trees, where the maximum-degree node is
/// usually not the root. Components are taken in ascending dense index, which is the
/// reference's `G.nodes()` order.
fn component_roots(graph: &SimpleGraph, n: u32) -> Vec<u32> {
    let mut seen = vec![false; n as usize];
    let mut levels = vec![UNLEVELED; n as usize];
    let mut parents = vec![UNLEVELED; n as usize];
    let mut roots = Vec::new();
    for start in 0..n {
        if seen[start as usize] {
            continue;
        }
        let first = sweep(graph, start, &mut levels, &mut parents);
        for &node in &first {
            seen[node as usize] = true;
        }
        // `order[-1]` is the last node the first sweep reached, the reference's farthest
        // guess; the second sweep starts there and its own last node is the far end.
        let far = sweep(
            graph,
            *first.last().expect("one node"),
            &mut levels,
            &mut parents,
        );
        let mut path = vec![*far.last().expect("one node")];
        while parents[path[path.len() - 1] as usize] != UNLEVELED {
            path.push(parents[path[path.len() - 1] as usize]);
        }
        roots.push(path[path.len() / 2]);
    }
    roots
}

/// One `_bfs_levels` sweep (`hierarchical.py:16-29`) from `start`: the visit order, with
/// `levels` and `parents` filled for the nodes it reached and reset to [`UNLEVELED`]
/// before it returns, so the next component starts clean without an `O(n)` clear each.
fn sweep(graph: &SimpleGraph, start: u32, levels: &mut [u32], parents: &mut [u32]) -> Vec<u32> {
    let mut order = vec![start];
    levels[start as usize] = 0;
    parents[start as usize] = UNLEVELED;
    let mut head = 0;
    while head < order.len() {
        let node = order[head];
        head += 1;
        let depth = levels[node as usize] + 1;
        for edge in graph.rows.row(node) {
            let next = graph.other(*edge, node);
            if levels[next as usize] == UNLEVELED {
                levels[next as usize] = depth;
                parents[next as usize] = node;
                order.push(next);
            }
        }
    }
    for &node in &order {
        levels[node as usize] = UNLEVELED;
    }
    order
}

/// `_multi_source_levels` (`hierarchical.py:54-81`): BFS depth from every root at once,
/// and a node no root reaches seeds a level zero of its own so that every node ends up
/// with a level. Returns `(node, level)` in BFS discovery order, which is the order
/// `levels` is inserted in and therefore the order `_group_by_level` groups by.
fn multi_source_levels(graph: &SimpleGraph, roots: &[u32], n: u32) -> Vec<(u32, u32)> {
    let mut levels = vec![UNLEVELED; n as usize];
    let mut queue: Vec<u32> = Vec::with_capacity(n as usize);
    for &root in roots {
        if levels[root as usize] == UNLEVELED {
            levels[root as usize] = 0;
            queue.push(root);
        }
    }
    let mut head = 0;
    let mut pending = 0u32;
    loop {
        head = drain(graph, &mut queue, &mut levels, head);
        let Some((node, next)) = reseed(&levels, pending) else {
            break;
        };
        pending = next;
        levels[node as usize] = 0;
        queue.push(node);
    }
    queue
        .iter()
        .map(|&node| (node, levels[node as usize]))
        .collect()
}

/// The reference's inner `while head < len(queue)` sweep, one round of it: every node the
/// frontier holds is levelled one deeper and enqueued. Returns the new head, which is the
/// queue's length once the frontier is drained — the queue is never popped from, so the
/// insertion order `levels` is read back in is the discovery order (D3).
fn drain(graph: &SimpleGraph, queue: &mut Vec<u32>, levels: &mut [u32], mut head: usize) -> usize {
    while head < queue.len() {
        let node = queue[head];
        head += 1;
        let depth = levels[node as usize] + 1;
        for edge in graph.rows.row(node) {
            let next = graph.other(*edge, node);
            if levels[next as usize] == UNLEVELED {
                levels[next as usize] = depth;
                queue.push(next);
            }
        }
    }
    head
}

/// The reference's `for node in pending: ... break / else: return` (`hierarchical.py:75-80`),
/// as a cursor over dense index: the lowest-index node no sweep has reached, and the cursor
/// past it. `None` is the `else: return`, so a re-seed never re-scans what it already passed.
fn reseed(levels: &[u32], cursor: u32) -> Option<(u32, u32)> {
    let mut node = cursor;
    while node < levels.len() as u32 {
        if levels[node as usize] == UNLEVELED {
            return Some((node, node + 1));
        }
        node += 1;
    }
    None
}

/// `_circular_hierarchy_layout`'s last loop (`hierarchical.py:718-729`): one ring per
/// level, node `i` of `count` at `(i / count) * 2 * pi` in the order its level was reached.
fn points(levels: &[(u32, u32)], n: u32) -> (Vec<f64>, Vec<f64>) {
    let mut width = 0u32;
    for &(_, level) in levels {
        width = width.max(level + 1);
    }
    let mut rings: Vec<Vec<u32>> = vec![Vec::new(); width as usize];
    for &(node, level) in levels {
        rings[level as usize].push(node);
    }
    let deepest = width.saturating_sub(1);
    let (mut x, mut y) = (vec![0.0; n as usize], vec![0.0; n as usize]);
    for (level, ring) in rings.iter().enumerate() {
        let count = ring.len() as f64;
        let radius = ring_radius(level as u32, ring.len(), deepest);
        for (slot, &node) in ring.iter().enumerate() {
            let angle = (slot as f64 / count) * (2.0 * core::f64::consts::PI);
            x[node as usize] = radius * libm::cos(angle);
            y[node as usize] = radius * libm::sin(angle);
        }
    }
    (x, y)
}

/// The reference's own radius rule (`hierarchical.py:722-726`): a lone level-zero node sits
/// on the axis, and every other ring — level 0 with two or more nodes included — is
/// `max(level, 0.35) * scale / max(2, max_level)`. The `0.35` floor is what keeps several
/// roots on a ring inside level one.
fn ring_radius(level: u32, count: usize, deepest: u32) -> f64 {
    if level == 0 && count == 1 {
        return 0.0;
    }
    f64::from(level).max(0.35) * SCALE / f64::from(deepest.max(2))
}

#[cfg(test)]
mod tests;
