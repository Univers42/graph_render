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
use crate::analysis::communities;
use crate::index::Topology;
use std::collections::BTreeMap;

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

/// Maximal runs of degree-2 nodes between two branch nodes (degree ≠ 2) contract to one
/// representative-level edge. A run that closes on itself is a bare ring: there is no
/// second endpoint to imply an edge to, so it is left alone rather than half-drawn.
fn contract_chains(t: &Topology, graph: &Simple, out: &mut Simplified) {
    let mut interior = vec![false; t.node_count() as usize];
    for start in 0..t.node_count() {
        if graph.degree(start) != 2 || out.visible[start as usize] == 0 || interior[start as usize]
        {
            continue;
        }
        let (lo, hi, path) = walk_chain(graph, start);
        if lo == hi || interior[lo as usize] || interior[hi as usize] {
            continue;
        }
        for &node in &path {
            interior[node as usize] = true;
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

/// The chain's own edges: the path `lo, path…, hi`, edge by edge, ascending. A parallel
/// edge on one hop is removed with it — the hop is gone, so every copy of it is.
fn chain_edges(t: &Topology, path: &[u32], lo: u32, hi: u32) -> Vec<u32> {
    let mut walk: Vec<u32> = Vec::with_capacity(path.len() + 2);
    walk.push(lo);
    walk.extend_from_slice(path);
    walk.push(hi);
    let ends: Vec<(u32, u32)> = walk.windows(2).map(|w| (w[0], w[1])).collect();
    let mut edges: Vec<u32> = (0..t.edge_count())
        .filter(|&e| {
            let (a, b) = (t.edges().source[e as usize], t.edges().target[e as usize]);
            ends.iter()
                .any(|&(u, v)| (a == u && b == v) || (a == v && b == u))
        })
        .collect();
    edges.sort_unstable();
    edges.dedup();
    edges
}

/// Every community collapses into its lowest dense index; internal edges go, external
/// edges are re-anchored on the representatives as [`Step::links`].
///
/// Two phases, and the order matters: every node's representative is assigned *before*
/// any edge is classified, so an edge from a member to its own representative reads as
/// internal rather than as a link to a node that is about to disappear. Communities are
/// then journalled in ascending community id, so the journal's order is a function of
/// the partition and not of a hash map's iteration order (D2).
fn collapse_communities(t: &Topology, out: &mut Simplified) {
    let membership = communities::louvain(t);
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (node, &community) in membership.iter().enumerate() {
        groups.entry(community).or_default().push(node as u32);
    }
    for members in groups.values() {
        let representative = members[0];
        for &node in members {
            out.representative[node as usize] = representative;
            // A later pass never resurrects what an earlier one removed: the leaf pass
            // ran first, and a node it folded stays folded even if it is its community's
            // lowest index.
            if node != representative && out.visible[node as usize] == 1 {
                out.visible[node as usize] = 0;
            }
        }
    }
    for members in groups.values() {
        collapse_one(t, members, out);
    }
}

/// One community's collapse, or nothing when it is a singleton: there is nothing to
/// collapse, and a step with no members would be a step that removes nothing.
fn collapse_one(t: &Topology, members: &[u32], out: &mut Simplified) {
    let representative = members[0];
    let removed: Vec<u32> = members
        .iter()
        .copied()
        .filter(|&node| node != representative)
        .collect();
    if removed.is_empty() {
        return;
    }
    let mut edges = Vec::new();
    let mut links: Vec<(u32, u32)> = Vec::new();
    for e in 0..t.edge_count() {
        let (a, b) = (t.edges().source[e as usize], t.edges().target[e as usize]);
        let (ra, rb) = (
            out.representative[a as usize],
            out.representative[b as usize],
        );
        if ra == rb {
            // Internal to this community: this step removes it. Internal to another
            // community: that community's own step removes it, and recording it here
            // would both double-count it and invent a self-link.
            if same_community(members, ra) {
                edges.push(e);
            }
        } else {
            links.push((ra.min(rb), ra.max(rb)));
        }
    }
    for &edge in &edges {
        out.edges[edge as usize] = 0;
    }
    links.sort_unstable();
    links.dedup();
    out.steps.push(Step {
        kind: Kind::Community,
        representative,
        nodes: removed,
        edges,
        links,
    });
}

/// Whether `representative` is the surviving node of `members` — an edge whose endpoints
/// share a representative is internal to *this* community, not merely already collapsed.
fn same_community(members: &[u32], representative: u32) -> bool {
    members.contains(&representative)
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    /// A path of 6: nodes 0..6 with 0 and 5 the only branch nodes, 1..4 degree 2.
    fn path6() -> Topology {
        let nodes: Vec<_> = (0..6).map(|i| node(&format!("n{i}"), "db")).collect();
        let edges: Vec<_> = (1..6)
            .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
            .collect();
        index_model(&nodes, &edges).expect("fits")
    }

    /// The gate row, in the shape the phase's gate runs it: whatever the plan removed,
    /// [`restore`] gives the original graph back, node for node and edge for edge, from
    /// the journal alone.
    #[test]
    fn simplify_reversible_restores_the_original_nodes_edges_and_representatives() {
        let t = path6();
        let original = (
            vec![1_u8; t.node_count() as usize],
            vec![1_u8; t.edge_count() as usize],
            (0..t.node_count()).collect::<Vec<u32>>(),
        );
        for plan in [
            Plan::nothing(),
            Plan::all(),
            Plan {
                fold_leaves: true,
                ..Plan::nothing()
            },
            Plan {
                contract_chains: true,
                ..Plan::nothing()
            },
            Plan {
                collapse_communities: true,
                ..Plan::nothing()
            },
        ] {
            let s = simplify(&t, &plan);
            let back = restore(&s);
            assert_eq!(
                (back.visible, back.edges, back.representative),
                original,
                "{plan:?}"
            );
        }
    }

    /// Reversibility on a graph with every kind of node in it: leaves, a chain, a ring
    /// and a dense core, so each pass has something to remove and nothing overlaps.
    #[test]
    fn simplify_reversible_holds_on_a_mixed_graph() {
        let mut nodes: Vec<_> = (0..15).map(|i| node(&format!("n{i}"), "db")).collect();
        nodes.push(node("leaf", "db"));
        let ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
        let pairs = [
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 0), // a ring of four: no branch node to contract to
            (0, 4),
            (4, 5),
            (5, 0), // a triangle
            (0, 6),
            (6, 7),
            (7, 0), // another triangle
            (1, 8),
            (8, 9),
            (9, 1), // another
            (2, 10),
            (10, 11),
            (11, 2), // another
            (3, 12),
            (12, 13),
            (13, 3), // another
            (4, 15), // node 15 is a leaf
        ];
        let edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| edge(&format!("e{i}"), &ids[*a as usize], &ids[*b as usize]))
            .collect();
        let t = index_model(&nodes, &edges).expect("fits");
        let s = simplify(&t, &Plan::all());
        let back = restore(&s);
        assert_eq!(back.visible, vec![1; t.node_count() as usize]);
        assert_eq!(back.edges, vec![1; t.edge_count() as usize]);
        assert_eq!(
            back.representative,
            (0..t.node_count()).collect::<Vec<u32>>()
        );
        assert_eq!(s.visible[15], 0, "the leaf was folded");
        assert!(
            counts(&s).0 > 0,
            "something was reduced, or the graph proves nothing"
        );
        let kinds: Vec<Kind> = s.steps.iter().map(|step| step.kind).collect();
        assert!(kinds.contains(&Kind::Leaf), "{kinds:?}");
        assert!(
            kinds.windows(2).all(|w| w[0] <= w[1]),
            "the journal is in pass order: {kinds:?}"
        );
        assert!(
            s.steps
                .iter()
                .all(|step| s.visible[step.representative as usize] == 1),
            "every step names a node that survived"
        );
        assert!(
            s.steps
                .iter()
                .all(|step| step.nodes.iter().all(|&node| s.visible[node as usize] == 0))
        );
    }

    /// A contracted chain looks like an edge: the interior nodes go, the two branch
    /// nodes stay, and the journal is what says they are now adjacent.
    #[test]
    fn a_contracted_chain_is_one_journalled_link_between_its_branch_nodes() {
        let t = path6();
        let s = simplify(
            &t,
            &Plan {
                contract_chains: true,
                ..Plan::nothing()
            },
        );
        let step = s.steps.first().expect("the chain was contracted");
        assert_eq!(step.kind, Kind::Chain);
        assert_eq!(step.nodes, vec![1, 2, 3, 4]);
        assert_eq!(step.links, vec![(0, 5)]);
        assert_eq!(step.edges, vec![0, 1, 2, 3, 4]);
        assert_eq!(s.visible, vec![1, 0, 0, 0, 0, 1]);
        assert_eq!(s.edges, vec![0; 5]);
        assert_eq!(s.representative, vec![0, 0, 0, 0, 0, 5]);
    }

    /// A bare ring has no branch node, so it is left whole: half a contraction would draw
    /// fewer nodes than the graph has and imply an edge that does not exist.
    #[test]
    fn a_ring_of_degree_two_nodes_is_left_alone() {
        let nodes: Vec<_> = (0..4).map(|i| node(&format!("n{i}"), "db")).collect();
        let edges: Vec<_> = (0..4)
            .map(|i| {
                edge(
                    &format!("e{i}"),
                    &format!("n{i}"),
                    &format!("n{}", (i + 1) % 4),
                )
            })
            .collect();
        let t = index_model(&nodes, &edges).expect("fits");
        let s = simplify(
            &t,
            &Plan {
                contract_chains: true,
                ..Plan::nothing()
            },
        );
        assert!(s.steps.is_empty(), "{:?}", s.steps);
        assert_eq!(s.visible, vec![1; 4]);
    }

    /// Communities collapse to their lowest dense index, and the links the journal
    /// carries are the external edges re-anchored there — the drill-back a front needs.
    #[test]
    fn a_community_collapses_onto_its_lowest_index_with_its_external_links() {
        // Two triangles joined by one edge: two communities, joined at 2 and 5.
        let nodes: Vec<_> = (0..6).map(|i| node(&format!("n{i}"), "db")).collect();
        let pairs = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (2, 5)];
        let edges: Vec<_> = pairs
            .iter()
            .enumerate()
            .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
            .collect();
        let t = index_model(&nodes, &edges).expect("fits");
        let s = simplify(
            &t,
            &Plan {
                collapse_communities: true,
                ..Plan::nothing()
            },
        );
        assert_eq!(s.steps.len(), 2, "two communities, both of size three");
        assert!(s.steps.iter().all(|step| step.kind == Kind::Community));
        let first = &s.steps[0];
        assert_eq!(first.representative, 0);
        assert_eq!(first.nodes, vec![1, 2]);
        assert_eq!(
            first.links,
            vec![(0, 3)],
            "the joining edge, re-anchored on 0"
        );
        assert_eq!(first.edges, vec![0, 1, 2], "the triangle's own three edges");
        assert_eq!(
            s.visible,
            vec![1, 0, 0, 1, 0, 0],
            "one node per community survives"
        );
        assert_eq!(s.edges[6], 1, "the joining edge itself is still drawn");
        assert_eq!(s.edges[0], 0, "an edge inside a collapsed community is not");
    }

    /// Nothing is removed unless a pass is asked for, and the identity plan is the
    /// identity: the same graph, the same journal length, node for node.
    #[test]
    fn the_identity_plan_removes_nothing_and_the_passes_are_ordered() {
        let t = path6();
        let none = simplify(&t, &Plan::nothing());
        assert!((none.steps.is_empty(), counts(&none)) == (true, (0, 0)));
        assert_eq!(none.visible, vec![1; 6]);
        let all = simplify(&t, &Plan::all());
        let kinds: Vec<Kind> = all.steps.iter().map(|s| s.kind).collect();
        assert!(kinds.windows(2).all(|w| w[0] <= w[1]), "{kinds:?}");
    }
}
