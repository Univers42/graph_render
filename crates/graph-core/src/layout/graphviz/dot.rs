//! Graphviz 16.1.0's `dot`: the layered (hierarchical) engine, in the order its four
//! passes run.
//!
//! Reference: `lib/dotgen/dotinit.c:301-340` (`dotLayout`), which is the whole contract:
//!
//! 1. `dot_rank` — `acyclic`, then one network simplex per connected component. **Ported**;
//!    see [`rank`].
//! 2. `dot_mincross` — `build_ranks` for the initial order, then median/transpose passes.
//!    **Ported**; see [`mincross`].
//! 3. `dot_position` — y from the rank heights, then a second network simplex over an
//!    auxiliary graph for x, then the frame `-Tplain` prints. **Ported**; see [`position`].
//! 4. `dot_splines` — edges as splines through the virtual nodes. Not needed: the motor
//!    emits polylines through the virtual nodes.
//!
//! **What is and is not reproduced.** The three passes are ported and the engine draws a graph.
//! What is *not* reproduced is byte-exact agreement with Graphviz's own drawing, and the
//! measurement says why in numbers: over the 1000 seeded fixtures 692 agree node for node on the
//! ranks and 408 on every rank's order, which is the *face* of the simplex's optimum rather
//! than a defect — two correct implementations reach different vertices of one optimal face.
//! `docs/measurements/p13-gv2-dot.md` has the counts and the position pass's own sweep.
//!
//! The node box Graphviz gives a node is sized from its *rendered label*, and that width is an
//! input to the x-coordinate network simplex. The motor ships no font engine, so the width comes
//! from the measured table in [`layout::graphviz::text_width`](super::text_width) through
//! [`node_box`] — that file is the only source of node widths this port uses, and
//! `docs/decisions/graphviz-oracle.md` records the decision.
//!
//! The module doc of `layout::graphviz` says why these engines are reimplemented rather
//! than translated; the two ports next to this one, `osage` and `patchwork`, are the same shape.
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
pub mod mincross;
pub mod position;
pub mod rank;
pub mod simplex;

#[cfg(test)]
mod class2_tests;
#[cfg(test)]
mod mincross_tests;
#[cfg(test)]
mod oracle_crossings;
#[cfg(test)]
mod oracle_digest;
#[cfg(test)]
mod oracle_probe;
#[cfg(test)]
mod order_tests;
#[cfg(test)]
mod position_tests;
#[cfg(test)]
mod rank_fixture_edges;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod tests;

use decomp::decompose;
use fast::{Edge, Fast, Node};
use super::text_width as measured;

pub use position::position;
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

/// The width every node gets from its own id, in points: `node_width(text_width(id))`.
///
/// This is where the port leaves Graphviz's font engine. Graphviz sizes a node from its
/// *rendered* label, and the rendered width is a metric of a font the motor does not ship;
/// `layout::graphviz::text_width` is the measured table that stands in for it, and this
/// function is the only place that table reaches a drawing. The relation is exact for the
/// labels the fixtures carry (`n` and ASCII digits, up to four characters, measured — see
/// `text_width`'s own caveat for the rest).
pub fn node_box(id: &str) -> (f64, f64) {
    let width = measured::node_width(measured::text_width(id));
    (width / 2.0, width / 2.0)
}

/// A fast graph with every node given Graphviz's default box and no edges yet.
///
/// The box is the default rather than the width the node's own label needs, because the label
/// is not known here: [`node_box`] is the sized version, and [`build`] is where a real id gets
/// in. Every fixture node of the first ten fits the default anyway, which is why the closed
/// cases and seeds 0 to 9 are the same drawing either way.
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

/// A fast graph with every node sized from its own id and the given edges in.
///
/// The rank and mincross passes do not read a node's width, so the sized and the default box
/// give the same ranks and the same order. The position pass reads it, as a constraint length,
/// so this is where `text_width` becomes a drawing.
pub fn build(ids: &[&str], edges: &[(u32, u32)]) -> Fast {
    let mut g = Fast::new();
    for id in ids {
        let (lw, rw) = node_box(id);
        g.add_node(Node::normal(0, lw, rw, NODE_H));
    }
    add_edges(&mut g, edges);
    g
}
