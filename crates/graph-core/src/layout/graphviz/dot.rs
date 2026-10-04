//! Graphviz 16.1.0's `dot`: the layered (hierarchical) engine, in the order its four
//! passes run.
//!
//! Reference: `lib/dotgen/dotinit.c:301-340` (`dotLayout`), which is the whole contract:
//!
//! 1. `dot_rank` — `acyclic`, then one network simplex per connected component. **Ported**;
//!    see [`mod@rank`].
//! 2. `dot_mincross` — `build_ranks` for the initial order, then median/transpose passes.
//!    Not ported; [`class2`], the edge classification this pass needs, is.
//! 3. `dot_position` — y from the rank heights, then a second network simplex over an
//!    auxiliary graph for x. Not ported; the engine it needs, [`simplex`], is.
//! 4. `dot_splines` — edges as splines through the virtual nodes. Not needed: the motor
//!    emits polylines through the virtual nodes.
//!
//! The node box Graphviz gives a node is sized from its *rendered label*, and that width is
//! an input to the x-coordinate network simplex — which is why the port reaches the rank
//! pass and stops, and why the width itself is pinned in `layout::graphviz::text_width`.
//! `docs/measurements/p13-gv2-dot.md` has the measurement and `docs/decisions/graphviz-oracle.md`
//! the decision.
//!
//! The module doc of `layout::graphviz` says why these engines are reimplemented rather
//! than translated, and `docs/decisions/graphviz-oracle.md` records that decision; the
//! two ports next to this one, `osage` and `patchwork`, are the same shape.
//!
//! Determinism: the port's own order is the dense node index throughout; the oracle is
//! **not** seed-sensitive — `docs/measurements/p13-gv2-dot.md` records the same fixture
//! hashing identically at `-Gstart` 1, 7 and 99 over all 1000 seeds — so a difference
//! from Graphviz is an algorithmic difference, never drift.

pub mod acyclic;
pub mod class1;
pub mod class2;
pub mod decomp;
pub mod fast;
pub mod rank;
pub mod simplex;

#[cfg(test)]
mod class2_tests;
#[cfg(test)]
mod oracle_probe;
#[cfg(test)]
mod rank_fixture_edges;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod tests;

use decomp::decompose;
use fast::{Edge, Fast, Node};

pub use rank::rank;
pub use simplex::Error;

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
///
/// The box is the default rather than the width the node's own label needs, because the
/// label is not known here and the default is what every fixture node below ten nodes gets
/// anyway. `layout::graphviz::text_width` has the measured relation, and
/// `p13-gv2-dot-position` is where the box is settled.
pub fn empty_graph(count: u32) -> Fast {
    let mut g = Fast::new();
    for _ in 0..count {
        g.add_node(Node::normal(0, NODE_W / 2.0, NODE_W / 2.0, NODE_H));
    }
    g
}

/// Add the **input** edges `(from, to)`, in the order given.
///
/// The reference reads a DOT graph, where an edge's direction is the order its endpoints
/// are written in; the motor's edge list is in dense-index order and that order is the
/// direction. This records the input graph only — [`class1::run`] is what puts edges into the
/// fast graph the ranking pass walks.
pub fn add_edges(g: &mut Fast, edges: &[(u32, u32)]) {
    for &(from, to) in edges {
        g.add_edge(Edge::real(from, to));
    }
}

/// The first two stages of `dot1_rank` (`rank.c:509-518`): `class1`, then `decompose`, then
/// `acyclic` on every component. Returns the components in `decompose`'s order, which is the
/// order the network simplex will be run in. This is the entry the cycle-breaking tests use,
/// because cycle breaking is the first thing that can be checked without a rank.
pub fn break_cycles(g: &mut Fast) -> Vec<Vec<u32>> {
    class1::run(g);
    let components = decompose(g);
    for component in &components {
        acyclic::run(g, component);
    }
    components
}
