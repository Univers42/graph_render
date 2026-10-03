//! `layout.hierarchical3d`: SciGraphs' own `_hierarchical_layout_3d`
//! (`SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:113-147`), ported whole.
//! One disk per BFS level, filled in order, the level on `z` and the disk in `x`/`y`;
//! `Point` nodes, straight `Line` edges, `O(n + m)`, no RNG and no iteration.
//!
//! **The reference's own docstring is stale and the code is authoritative.** It claims
//! "BFS depth sets Y, each level fills a disk in XZ"; the code puts the level on `z`
//! (`hierarchical.py:140`, `:144`) and the disk in `x`/`y`, so this port does. A port that
//! followed the prose would disagree with the reference on every single node.
//!
//! **A directed input is read undirected, and that is the port's one stated departure.**
//! SciGraphs branches on `G.is_directed()` (`hierarchical.py:127-128`): a digraph's roots
//! are its in-degree-0 **sources**, and `neighbors` then means successors, which also
//! changes what a level *is*. The motor's [`Topology`] carries directedness per edge
//! (`edges().directed`) and has no whole-graph flag, so there is no such predicate to
//! branch on — the sibling port documents this at
//! `crates/graph-core/src/registry/hierarchy.rs` and
//! `crates/graph-cli/src/oracle_python/circular_hierarchy.rs:11-16`. This port therefore
//! takes the **undirected** branch, `_component_roots` (`hierarchical.py:31-52`), and does
//! not port the in-degree-0 root branch, its no-source fallback, or successor-only
//! traversal. Escape hatch: repair the digraph into a forest upstream, where the caller
//! already owns the direction.
//!
//! **A level is this module's own undirected BFS depth, not a layer of the 2D Sugiyama
//! layout.** Nothing here reads `layout::sugiyama`: the levels come from
//! [`levels::component_roots`] plus [`levels::multi_source_levels`] (`hierarchical.py:31-52`
//! and `:66-81`), so on a digraph they are BFS distances in the undirected projection
//! while `sugiyama`'s `layering::assign_layers` would give longest-path layers. The two
//! disagree wherever BFS depth and longest-path depth do — the chain `1 -> 0 -> 2` gets
//! Sugiyama layers `1: 0, 0: 1, 2: 2` and this port BFS depths `0: 0, 1: 1, 2: 1`, because
//! its root is the component's diameter midpoint rather than an in-degree-0 source. That is
//! the reference's definition (its oracle is SciGraphs, and conformance row 17 is green
//! node for node), so the fix is this sentence and not the arithmetic: this layout is an
//! independent port of `_hierarchical_layout_3d`, never a lift of the 2D result, and a
//! caller that needs the Sugiyama layering must run that layout.
//!
//! **Order is imposed, because the reference's is a `dict`'s.** `_group_by_level`
//! (`hierarchical.py:83-88`) builds `{level: [node, ...]}` by iterating a `dict`; the port
//! iterates **ascending level** and, inside a level, **BFS discovery order** (D2/D3). That
//! is not a replacement but the same sequence: CPython dicts are insertion-ordered, and
//! `levels` is filled in BFS discovery order, whose levels are non-decreasing apart from a
//! re-seed at 0, so a level key is first created before any larger one and dict order and
//! ascending level agree. Ties never reach a `HashMap` here: every table below is indexed
//! by dense index or by level.
//!
//! **Two numpy behaviours are load-bearing and spelled out rather than approximated.**
//!
//! * `np.round` / Python's `round` is **ties to even**. [`disk::half_to_even`] is that rule;
//!   `f64::round` is ties *away from zero* and disagrees at every exact half. At `count =
//!   10` the reference's own `take` is `np.round([2.5, 7.5]) = [2, 8]`, where a naive round
//!   gives `[3, 8]`, the reference's own `while take.sum() > count` loop then moves one unit
//!   to the argmax, and the drawing comes out `[3, 7]` — a different picture, not a
//!   different rounding.
//! * `np.argmax` returns the **first** maximum, so a tie between two rings takes the lower
//!   ring index ([`disk`]'s `argmax`). `Iterator::max` would take the last.
//!
//! **The `widest` guard.** The reference guards `z`'s denominator with `max(1, max_level)`
//! and uses `widest` bare (`hierarchical.py:140`, `:142`). `widest` cannot be 0 on this
//! path: `num_nodes == 0` returns at `:123`, every other node gets a level from
//! `_multi_source_levels` (`hierarchical.py:66-81`), so `grouped` is non-empty and some
//! level holds at least one node. The port still applies `max(1)`: it is a provable no-op
//! that makes the division total without a second reasoning chain, the same convention as
//! the reference's own guard on the sibling denominator.
//!
//! **No `Params` struct.** The reference takes `scale` alone and the dispatcher passes
//! `5.0`, so [`SCALE`] is published as a constant instead — the convention the sibling
//! `layout.circular.hierarchy` states (`layout/circular/hierarchy.rs:37-45`). The
//! capability id is [`ID`], which is not a `Stage::ID` for the same reason.
//!
//! The node set is the undirected, deduplicated, self-loop-free projection every force
//! layout shares ([`crate::layout::force::simple_graph`]), so a parallel pair and a 2-cycle
//! are one edge here, as they are one edge in the `nx.Graph` the differential builds.

use super::Geometry;
use crate::index::Topology;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

pub(crate) mod disk;
pub(crate) mod levels;

use disk::disk;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.hierarchical3d";

/// SciGraphs' `apply_graph_layout(scale=5.0)` default (`layouts/dispatcher.py:14`), the
/// value the dispatcher hands `_hierarchical_layout_3d`. The whole extent of the drawing:
/// `z` runs `[-scale, +scale]` and no disk is wider than `scale / 2`.
pub const SCALE: f64 = 5.0;

/// Runs the layout over `topology`'s undirected projection; never refuses, like the
/// reference it ports: every branch of it is total.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let graph: SimpleGraph = simple_graph(topology);
    let n = topology.node_count();
    let roots = levels::component_roots(&graph, n);
    let depths = levels::multi_source_levels(&graph, &roots, n);
    let (x, y, z) = place(&depths, n);
    // `in_space`, never `planar`: the z column is the only thing that makes the snapshot a
    // 3D one (`layout/mod.rs:75-87`), and losing it here would silently answer with a 0.3.
    Ok(Geometry::in_space(
        NodeGeometry::Point {
            x: narrow(&x),
            y: narrow(&y),
        },
        EdgeGeometry::Line,
        Vec::new(),
        narrow(&z),
    ))
}

/// `_hierarchical_layout_3d`'s placement loop (`hierarchical.py:139-144`): every level
/// takes its `z` and its radius, then the level's nodes take the disk's points in order.
///
/// An empty topology needs no branch of its own — the reference's `np.zeros((0, 3))` and
/// this empty triple are the same answer, and `widest` below is guarded anyway.
fn place(depths: &[(u32, u32)], n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let rings = levels::by_level(depths);
    let max_level = (rings.len() as u32).saturating_sub(1);
    let widest = rings.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let (mut x, mut y, mut z) = (
        vec![0.0; n as usize],
        vec![0.0; n as usize],
        vec![0.0; n as usize],
    );
    for (level, ring) in rings.iter().enumerate() {
        let depth = depth_z(level as u32, max_level);
        let radius = SCALE * 0.5 * f64::sqrt(ring.len() as f64 / widest as f64);
        for (&node, (px, py)) in ring.iter().zip(disk(ring.len(), radius)) {
            x[node as usize] = px;
            y[node as usize] = py;
            z[node as usize] = depth;
        }
    }
    (x, y, z)
}

/// `(level / max(1, max_level)) * scale * 2 - scale` (`hierarchical.py:140`): level 0 on
/// the negative end, the deepest level on the positive one. `max(1, max_level)` because a
/// lone node's deepest level is 0 and the reference's own division must stay total.
fn depth_z(level: u32, max_level: u32) -> f64 {
    (level as f64 / f64::from(max_level.max(1))) * SCALE * 2.0 - SCALE
}

/// The `f32` narrowing a snapshot's columns hold, applied once at the end so the whole
/// layout runs in `f64` and the rounding happens in one place.
fn narrow(column: &[f64]) -> Vec<f32> {
    column.iter().map(|&value| value as f32).collect()
}

#[cfg(test)]
mod tests;
