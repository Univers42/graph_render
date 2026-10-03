//! Community collapse: every Louvain community folds into its lowest drawn dense index.

use super::*;
use crate::analysis::communities;
use std::collections::BTreeMap;

/// Every community collapses into its lowest drawn dense index; internal edges go, and
/// each step's links are the external edges that touch it, re-anchored on the
/// representatives as [`Step::links`].
///
/// Three phases, each one pass, and the order matters (review R24: before, every
/// community rescanned every edge). Every representative is assigned *before* any edge
/// is classified, so an edge from a member to its own representative reads as internal
/// rather than as a link to a node that is about to disappear. Communities are journalled
/// in ascending community id, so the journal's order is a function of the partition and
/// not of a hash map's iteration order (D2).
pub(super) fn collapse_communities(t: &Topology, out: &mut Simplified) {
    let mut step_of: Vec<Option<usize>> = vec![None; t.node_count() as usize];
    let first = out.steps.len();
    for members in group_by_community(&communities::louvain(t)).values() {
        open_step(members, out, &mut step_of);
    }
    // A node an earlier pass removed points at a node that was drawn when this pass
    // began, and that node now points at a drawn one: one hop lands every node on a
    // drawn representative.
    for node in 0..out.representative.len() {
        let earlier = out.representative[node];
        out.representative[node] = out.representative[earlier as usize];
    }
    reanchor_links(&mut out.steps[..first], &out.representative);
    classify_edges(t, &step_of, out);
    for step in &mut out.steps[first..] {
        step.links.sort_unstable();
        step.links.dedup();
    }
}

/// Every step an earlier pass already journalled has its links re-anchored on the
/// representatives that exist now that the fixup above has landed: a chain link `(2, 5)`
/// whose `2` turned out to be in representative `0` becomes `(0, 5)`. Without this the
/// drill-back a front reads names a node it no longer draws.
///
/// A link whose two ends land in the **same** representative goes: it names one node
/// twice, which is not an edge. [`classify_edges`] never writes one — it only pushes when
/// `ra != rb` — and neither does the reference's coarse level, whose super-edges are the
/// `inter` ones (`simplify.py:216-217`). Ascending and deduplicated, because two links can
/// land on the same pair once collapsed.
fn reanchor_links(steps: &mut [Step], representative: &[u32]) {
    for step in steps {
        for link in &mut step.links {
            let (a, b) = (
                representative[link.0 as usize],
                representative[link.1 as usize],
            );
            *link = (a.min(b), a.max(b));
        }
        step.links.retain(|&(a, b)| a != b);
        step.links.sort_unstable();
        step.links.dedup();
    }
}

/// Each community's members, ascending, keyed by ascending community id.
fn group_by_community(membership: &[u32]) -> BTreeMap<u32, Vec<u32>> {
    let mut groups: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (node, &community) in membership.iter().enumerate() {
        groups.entry(community).or_default().push(node as u32);
    }
    groups
}

/// One community's step: its drawn members but the lowest go into the lowest. Nothing
/// when fewer than two members are drawn: there is nothing to collapse, and a later
/// pass never resurrects what an earlier one removed, nor represents a community by a
/// node it no longer draws (review U25).
fn open_step(members: &[u32], out: &mut Simplified, step_of: &mut [Option<usize>]) {
    let mut drawn = members
        .iter()
        .copied()
        .filter(|&node| out.visible[node as usize] == 1);
    let Some(representative) = drawn.next() else {
        return;
    };
    let removed: Vec<u32> = drawn.collect();
    if removed.is_empty() {
        return;
    }
    for &node in &removed {
        out.visible[node as usize] = 0;
        out.representative[node as usize] = representative;
    }
    step_of[representative as usize] = Some(out.steps.len());
    out.steps.push(Step {
        kind: Kind::Community,
        representative,
        nodes: removed,
        edges: Vec::new(),
        links: Vec::new(),
    });
}

/// Every edge once, by its endpoints' representatives. A drawn edge inside one step's
/// representative is that step's to remove; an edge between two representatives is a
/// link of each end's step. An edge an earlier pass removed still names its link (a
/// contracted chain joins the communities of its two ends) but is not removed twice.
fn classify_edges(t: &Topology, step_of: &[Option<usize>], out: &mut Simplified) {
    let edges = t.edges();
    for e in 0..t.edge_count() {
        let (a, b) = (edges.source[e as usize], edges.target[e as usize]);
        let (ra, rb) = (
            out.representative[a as usize],
            out.representative[b as usize],
        );
        if ra != rb {
            let link = (ra.min(rb), ra.max(rb));
            for step in [step_of[ra as usize], step_of[rb as usize]]
                .into_iter()
                .flatten()
            {
                out.steps[step].links.push(link);
            }
        } else if let (1, Some(step)) = (out.edges[e as usize], step_of[ra as usize]) {
            out.edges[e as usize] = 0;
            out.steps[step].edges.push(e);
        }
    }
}
