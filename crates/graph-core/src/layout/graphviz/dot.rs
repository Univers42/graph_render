//! Graphviz 16.1.0's `dot`: the layered (hierarchical) engine, in the order its four
//! passes run.
//!
//! Reference: `lib/dotgen/dotinit.c:301-340` (`dotLayout`), which is the whole contract:
//!
//! 1. `dot_rank` — `acyclic`, then one network simplex per connected component.
//! 2. `dot_mincross` — `build_ranks` for the initial order, then median/transpose passes.
//! 3. `dot_position` — y from the rank heights, then a second network simplex over an
//!    auxiliary graph for x.
//! 4. `dot_splines` — edges as splines through the virtual nodes.
//!
//! **Only pass 0 of step 1 and the component decomposition are ported here.** See
//! `docs/measurements/p13-gv2-dot.md` for what is measured, what is not, and why: the
//! node box Graphviz gives a node is sized from its *rendered label*, and that width is a
//! font metric of Graphviz's own text layout (measured: 54 points for a two-character id
//! and 57.942 for a three-character one). It is the input to the x-coordinate network
//! simplex, so no amount of care in the three remaining passes reproduces Graphviz's x
//! coordinates without a font engine, and a `layout.dag.dot` row claiming otherwise would
//! be a claim this repository cannot support.
//!
//! The module doc of `layout::graphviz` says why these engines are reimplemented rather
//! than translated, and `docs/decisions/graphviz-oracle.md` records that decision; the
//! two ports next to this one, `osage` and `patchwork`, are the same shape and both carry
//! the label-width finding in their first Ponytail marker.
//!
//! Determinism: the port's own order is the dense node index throughout; the oracle is
//! **not** seed-sensitive — `docs/measurements/p13-gv2-dot.md` records the same fixture
//! hashing identically at `-Gstart` 1, 7 and 99 over all 1000 seeds — so a difference
//! from Graphviz is an algorithmic difference, never drift.

pub mod acyclic;
pub mod decomp;
pub mod fast;

#[cfg(test)]
mod tests;

use decomp::decompose;
use fast::{Edge, Fast, Node};

/// Graphviz's default node box, `0.75 x 0.5` inch (`const.h`'s `DEFAULT_NODEWIDTH` and
/// `DEFAULT_NODEHEIGHT`) in points, which is the whole conversion the layout needs.
pub const NODE_W: f64 = 0.75 * 72.0;
/// The default node height, in points.
pub const NODE_H: f64 = 0.5 * 72.0;
/// `DEFAULT_NODESEP` (`const.h:85`): the gap between two nodes on the same rank.
pub const NODESEP: f64 = 0.25 * 72.0;
/// `DEFAULT_RANKSEP` (`const.h:87`): the gap between two ranks.
pub const RANKSEP: f64 = 0.5 * 72.0;

/// A fast graph with every node given Graphviz's default box and no edges yet.
pub fn empty_graph(count: u32) -> Fast {
    let mut g = Fast::new();
    for _ in 0..count {
        g.add_node(Node::normal(0, NODE_W / 2.0, NODE_W / 2.0, NODE_H));
    }
    g
}

/// Add `count` real edges `(from, to)`, in the order given.
///
/// The reference reads a DOT graph, where an edge's direction is the order its endpoints
/// are written in; the motor's edge list is in dense-index order and that order is the
/// direction.
pub fn add_edges(g: &mut Fast, edges: &[(u32, u32)]) {
    for &(from, to) in edges {
        g.add_edge(Edge::real(from, to));
    }
}

/// `dot1_rank` (`rank.c:496-517`) as far as it is ported: decompose, then break every
/// cycle of every component. Returns the components in `decompose`'s order, which is the
/// order the network simplex will be run in.
pub fn break_cycles(g: &mut Fast) -> Vec<Vec<u32>> {
    let components = decompose(g);
    for component in &components {
        acyclic::run(g, component);
    }
    components
}
