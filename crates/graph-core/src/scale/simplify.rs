//! Topology reduction (`prompts/phase-09-scale-bench.md` §3): leaf folding, chain
//! contraction and community collapse, each one **reversible**.
//!
//! **Reversibility is the whole contract.** Every node and every edge a pass removes is
//! written into a [`Step`] — which nodes went, which edges went, into whose representative
//! they went, and which representative-level edges the step implies — so
//! [`restore`] puts the graph back exactly, and a front can drill in one community at a
//! time instead of re-deriving the graph it was given. An irreversible simplification is
//! data loss wearing an optimisation's name, and this module's `simplify_reversible_*`
//! tests are the gate row that says so.
//!
//! **What it changes, named:** a contracted chain *looks like an edge*. The interior nodes
//! are gone from the picture and their two endpoints are adjacent in the simplified graph
//! with no edge between them, so a front that ignores [`Step::links`] draws two isolated
//! nodes where a path was. The reversal path is [`restore`], and the drill-back path is
//! the journal itself: each step names its own representative and its members.
//!
//! **Ponytail: community collapse trusts [`crate::analysis::communities::louvain`], which
//! is a heuristic.** Failing input: a graph with near-tied modularity gains, or one whose
//! communities are chains of a single edge, where a collapse removes exactly the node a
//! reader came to see. Direction: cosmetic, because the journal still holds it — the
//! danger would be an *irreversible* collapse, which this module does not implement.
//! Escape hatch: [`Plan::collapse_communities`] off, and the other two passes still run.
//!
//! The reference (`SciGraphs/engine/scigraphs_engine/simplify.py`) extracts a *backbone*
//! (MST, disparity, top-k — three different notions of "the edges that matter") and
//! coarsens communities for hierarchical bundling. Neither is a simplification of the
//! graph itself and neither is reversible, so this module implements neither: it reduces
//! the node and edge *sets* and journals them, which is the operation the phase asks for.

use super::simple::{self, Simple};
use crate::index::Topology;

/// Which passes run. Every pass is off by default except leaf folding's own default
/// below; a caller that wants "nothing removed" passes [`Plan::nothing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Plan {
    /// Fold every degree-1 node into its neighbour.
    pub fold_leaves: bool,
    /// Contract maximal degree-2 chains into a single representative-level edge.
    pub contract_chains: bool,
    /// Collapse every community into its lowest dense index.
    pub collapse_communities: bool,
}

impl Plan {
    /// Every pass on: the phase's own reduction.
    pub fn all() -> Self {
        Self {
            fold_leaves: true,
            contract_chains: true,
            collapse_communities: true,
        }
    }

    /// Every pass off: the identity, so [`restore`] of it is trivially the original.
    pub fn nothing() -> Self {
        Self::default()
    }
}

/// Which pass removed something, so a front can label what it is looking at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A degree-1 node folded into its neighbour.
    Leaf,
    /// A maximal run of degree-2 nodes contracted between two branch nodes.
    Chain,
    /// A community collapsed into its lowest dense index.
    Community,
}

/// One reversible removal: what went, into what, and what the front should draw instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// The pass that made this step.
    pub kind: Kind,
    /// The surviving node this step's members now belong to.
    pub representative: u32,
    /// The nodes this step removed, ascending.
    pub nodes: Vec<u32>,
    /// The edges this step removed, ascending.
    pub edges: Vec<u32>,
    /// The representative-level edges this step implies, each `(lower, higher)`, ascending:
    /// what a front draws in place of what was removed. Empty for a leaf, whose
    /// representative is already an endpoint of the edge it kept.
    pub links: Vec<(u32, u32)>,
}

/// A simplified graph plus the journal that undoes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Simplified {
    /// Per node: `1` when the node is still drawn.
    pub visible: Vec<u8>,
    /// Per edge: `1` when the edge is still drawn.
    pub edges: Vec<u8>,
    /// Per node: the surviving node it now belongs to, itself when it survived.
    pub representative: Vec<u32>,
    /// Every removal, in pass order: leaves, then chains, then communities.
    pub steps: Vec<Step>,
}

/// The graph a simplification started from, rebuilt from the journal alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    /// Every node visible again.
    pub visible: Vec<u8>,
    /// Every edge present again.
    pub edges: Vec<u8>,
    /// Every node its own representative again.
    pub representative: Vec<u32>,
}

/// The simplification of `t` under `plan`: masks, representatives, and the journal.
pub fn simplify(t: &Topology, plan: &Plan) -> Simplified {
    let n = t.node_count() as usize;
    let graph = simple::build(t);
    let mut out = Simplified {
        visible: vec![1; n],
        edges: vec![1; t.edge_count() as usize],
        representative: (0..n as u32).collect(),
        steps: Vec::new(),
    };
    if plan.fold_leaves {
        fold_leaves(t, &graph, &mut out);
    }
    if plan.contract_chains {
        contract_chains(t, &graph, &mut out);
    }
    if plan.collapse_communities {
        collapse_communities(t, &mut out);
    }
    out
}

/// Puts back everything `s` removed: the graph `s` was made from, rebuilt from the
/// journal and from nothing else — no topology is retained inside `s` for the purpose.
pub fn restore(s: &Simplified) -> Restored {
    let mut visible = s.visible.clone();
    let mut edges = s.edges.clone();
    for step in &s.steps {
        for &node in &step.nodes {
            visible[node as usize] = 1;
        }
        for &edge in &step.edges {
            edges[edge as usize] = 1;
        }
    }
    let n = visible.len();
    Restored {
        visible,
        edges,
        representative: (0..n as u32).collect(),
    }
}

/// The nodes and edges the passes would remove, without building the journal: what a
/// front can ask before it decides to draw the simple or the reduced graph.
pub fn counts(s: &Simplified) -> (u32, u32) {
    (
        s.visible.iter().filter(|&&v| v == 0).count() as u32,
        s.edges.iter().filter(|&&e| e == 0).count() as u32,
    )
}

/// Degree-1 nodes fold into their only neighbour, lowest index first so a two-node graph
/// folds `1` into `0` and never the other way round.
fn fold_leaves(t: &Topology, graph: &Simple, out: &mut Simplified) {
    for v in 0..t.node_count() {
        if out.visible[v as usize] == 0 || graph.degree(v) != 1 {
            continue;
        }
        let neighbour = graph.row(v)[0];
        let representative = out.representative[neighbour as usize];
        let edges = edges_between(t, v, neighbour);
        for &edge in &edges {
            out.edges[edge as usize] = 0;
        }
        out.visible[v as usize] = 0;
        out.representative[v as usize] = representative;
        out.steps.push(Step {
            kind: Kind::Leaf,
            representative,
            nodes: vec![v],
            edges,
            links: Vec::new(),
        });
    }
}

use chain::contract_chains;
use community::collapse_communities;

mod chain;
mod community;
#[cfg(test)]
mod tests;

/// The ascending edge indices of the edges between `a` and `b`.
fn edges_between(t: &Topology, a: u32, b: u32) -> Vec<u32> {
    let mut edges: Vec<u32> = (0..t.edge_count())
        .filter(|&e| {
            let (s, g) = (t.edges().source[e as usize], t.edges().target[e as usize]);
            (s == a && g == b) || (s == b && g == a)
        })
        .collect();
    edges.sort_unstable();
    edges
}
