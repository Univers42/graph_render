//! Graphviz 16.1.0's `dot`: the layered (hierarchical) engine, in the order its four
//! passes run.
//!
//! Reference: `lib/dotgen/dotinit.c:301-340` (`dotLayout`), which is the whole contract:
//!
//! 1. `dot_rank` — `acyclic`, then one network simplex per connected component. **Ported**;
//!    see [`mod@rank`].
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
mod position_findings;
#[cfg(test)]
mod position_fixture_points;
#[cfg(test)]
mod position_steps;
#[cfg(test)]
mod position_tests;
#[cfg(test)]
mod rank_fixture_edges;
#[cfg(test)]
mod rank_tests;
#[cfg(test)]
mod tests;

use super::text_width as measured;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;
use decomp::decompose;
use fast::{Edge, Fast, Node};

pub use position::position;
pub use rank::rank;
pub use simplex::Error;

/// The capability id, and the hash gate's stage name. Appended to the registry's `LAYOUTS` last,
/// by the append-only rule the array's own header records.
pub const ID: &str = "layout.dag.dot";

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

/// `layout.dag.dot` at its defaults: the three ported passes over a `Topology`, in the
/// reference's order, and the point geometry they leave.
///
/// **No `Stage` impl and no `Params`, by the same decision `layout::radial::twopi` and
/// `layout.packing.osage` record**: the reference exposes `rankdir`, `nodesep`, `ranksep`,
/// `ratio`, `nslimit`, `searchsize` and the whole node-attribute channel, none of which the
/// ledger row or the oracle sets, so a `Params` here would buy a knob with nothing behind it.
///
/// The node box is the one input that does reach the drawing, and it comes from the measured
/// table through [`node_box`] — the port's whole substitute for Graphviz's font engine.
///
/// The reference's `dot_splines` is not run: this crate emits polylines through the chain
/// dummies rather than splines, so the fourth pass has nothing to add.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    if count == 0 {
        return Ok(point_geometry(&[], &[]));
    }
    let ids: Vec<String> = (0..count)
        .map(|i| topology.node_id(i).to_string())
        .collect();
    let borrowed: Vec<&str> = ids.iter().map(String::as_str).collect();
    let mut g = build(&borrowed, &input_edges(topology));
    rank(&mut g).map_err(|e| StageError::Param {
        name: "rank",
        rule: ranking_rule(e),
    })?;
    mincross::run(&mut g);
    position(&mut g).map_err(|e| StageError::Param {
        name: "position",
        rule: ranking_rule(e),
    })?;
    let mut x = vec![0.0; count as usize];
    let mut y = vec![0.0; count as usize];
    for node in 0..count as usize {
        x[node] = g.nodes[node].coord.x;
        y[node] = g.nodes[node].coord.y;
    }
    Ok(point_geometry(&x, &y))
}

/// The reference's own words for each way the two simplexes can refuse, so the refusal a caller
/// sees says which pass refused and why rather than surfacing a `Debug` of an enum.
fn ranking_rule(error: Error) -> &'static str {
    match error {
        Error::Disconnected => "the graph is not connected, so no spanning tree exists",
        Error::Tree => "the simplex tree was inconsistent, which is the reference's return 2",
        Error::Overflow => "a cut value did not fit a 32-bit int",
    }
}

/// The input edges in the order the topology declares them, as (tail, head).
///
/// A self-loop is dropped, for `class2`'s reason rather than the reference's: a self-loop takes
/// part in no pass here, and keeping it would put a node's own width into its own constraint.
fn input_edges(topology: &Topology) -> Vec<(u32, u32)> {
    let edges = topology.edges();
    edges
        .source
        .iter()
        .zip(edges.target.iter())
        .filter(|&(source, target)| source != target)
        .map(|(&source, &target)| (source, target))
        .collect()
}
