//! Community collapse: every Louvain community folds into its lowest dense index.

use super::*;
use crate::analysis::communities;
use std::collections::BTreeMap;

/// Every community collapses into its lowest dense index; internal edges go, external
/// edges are re-anchored on the representatives as [`Step::links`].
///
/// Two phases, and the order matters: every node's representative is assigned *before*
/// any edge is classified, so an edge from a member to its own representative reads as
/// internal rather than as a link to a node that is about to disappear. Communities are
/// then journalled in ascending community id, so the journal's order is a function of
/// the partition and not of a hash map's iteration order (D2).
pub(super) fn collapse_communities(t: &Topology, out: &mut Simplified) {
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
