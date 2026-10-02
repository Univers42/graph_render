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
use crate::layout::force::simple_graph;
use crate::layout::hierarchical_3d::levels;
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

/// Runs the layout over `topology`'s undirected projection; never refuses, like the
/// reference it ports: every branch of it is total.
///
/// The BFS half — `_component_roots`, `_multi_source_levels`, `_group_by_level` — is not
/// here: it is [`crate::layout::hierarchical_3d::levels`], shared verbatim with
/// `layout.hierarchical3d`, the other layout that ports the same SciGraphs functions.
/// `points` below is the only thing this port owns.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let graph = simple_graph(topology);
    let n = topology.node_count();
    let roots = levels::component_roots(&graph, n);
    let depths = levels::multi_source_levels(&graph, &roots, n);
    let (x, y) = points(&depths, n);
    Ok(point_geometry(&x, &y))
}

/// `_circular_hierarchy_layout`'s last loop (`hierarchical.py:718-729`): one ring per
/// level, node `i` of `count` at `(i / count) * 2 * pi` in the order its level was reached.
///
/// The grouping itself is [`levels::by_level`], the reference's `_group_by_level`; only the
/// angle and the radius are this port's own closed form.
fn points(depths: &[(u32, u32)], n: u32) -> (Vec<f64>, Vec<f64>) {
    let rings = levels::by_level(depths);
    let deepest = (rings.len() as u32).saturating_sub(1);
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
