//! The four named fixtures: the gallery graph, a rooted tree, a DAG and a bipartite graph.
//!
//! **One child module per seam the set already had.** Each of these four is one graph with
//! its own reading of `fixtures/*.json` and its own error text, while the gate model
//! (`gate_model.rs`) is generated rather than read. The node-order contract every fixture
//! here obeys is stated in [`super`].

use super::{Fixture, array, edge, from_repo, names, node, read};
use crate::runner::workspace_root;
use graph_core::NodeRecord;
use serde_json::Value;

/// SciGraphs' gallery graph: `fixtures/scigraphs/lesmis.json`, 77 nodes and 254 edges. The
/// one fixture whose node order is not ours to choose: `nodes[i].id` is the integer
/// SciGraphs' own run listed at position `i`, so position is preserved and only the *name*
/// changes to `n%04d`. A `nodes` array whose ids are not `0..n` is a hard error.
pub(super) fn lesmis() -> Result<Fixture, String> {
    let path = workspace_root().join("fixtures/scigraphs/lesmis.json");
    let doc = read(&path)?;
    let listed = array(&doc, "nodes", &path)?;
    let ids = names(listed.len());
    let mut seat: Vec<Option<usize>> = vec![None; listed.len()];
    for (position, entry) in listed.iter().enumerate() {
        let id = entry["id"]
            .as_u64()
            .ok_or_else(|| format!("{}: node {position} has no integer id", path.display()))?;
        let at = usize::try_from(id).map_err(|_| format!("node id {id} out of range"))?;
        if at >= seat.len() || seat[at].replace(position).is_some() {
            return Err(format!(
                "{}: node id {id} out of range or twice",
                path.display()
            ));
        }
    }
    if seat.iter().enumerate().any(|(id, at)| *at != Some(id)) {
        return Err(format!("{}: node ids are not 0..n", path.display()));
    }
    let mut edges = Vec::new();
    for (index, pair) in array(&doc, "edges", &path)?.iter().enumerate() {
        let ends: Vec<u64> = pair
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_default();
        if ends.len() != 2 {
            return Err(format!("{}: edge {index} is not a pair", path.display()));
        }
        edges.push(edge(index, &ids[ends[0] as usize], &ids[ends[1] as usize]));
    }
    let nodes = ids.iter().map(|id| node(id)).collect();
    Ok(Fixture {
        name: "lesmis",
        about: "SciGraphs' gallery graph: networkx.les_miserables_graph, 77 nodes, 254 edges",
        nodes,
        edges,
    })
}

/// One rooted tree: `fixtures/hierarchy/tree-balanced.json`, depth 3, 15 nodes. Its
/// `relates_to` edge is kept — a layout reads edges, not edge kinds, and dropping it would
/// make this a different graph from the one the repo already states it is.
pub(super) fn tree() -> Result<Fixture, String> {
    from_repo(
        "fixtures/hierarchy/tree-balanced.json",
        "tree-balanced",
        "the repo's balanced binary tree, depth 3, 15 nodes, every parent-first spelling",
    )
}

/// One DAG: `fixtures/dag/diamond.json`, the smallest graph with a real layering choice.
pub(super) fn dag() -> Result<Fixture, String> {
    from_repo(
        "fixtures/dag/diamond.json",
        "dag-diamond",
        "the repo's diamond: a->b,a->c,b->d,c->d, no dummy vertex and no crossing",
    )
}

/// One bipartite graph, `K(6, 8)`: 14 nodes, 48 edges.
///
/// The repo has no bipartite *generator* — `oracle_python/closed_form.rs:35-46` borrows its
/// partitions from our own output — so this is written out rather than generated. A complete
/// bipartite graph is the case where the partition rule and the placement rule are
/// separable, and the only fixture where that is true by construction.
pub(super) fn bipartite() -> Fixture {
    const LEFT: usize = 6;
    const RIGHT: usize = 8;
    let ids = names(LEFT + RIGHT);
    let nodes: Vec<NodeRecord> = ids.iter().map(|id| node(id)).collect();
    let mut edges = Vec::new();
    for a in 0..LEFT {
        for b in 0..RIGHT {
            edges.push(edge(edges.len(), &ids[a], &ids[LEFT + b]));
        }
    }
    Fixture {
        name: "bipartite",
        about: "K(6,8): 14 nodes, 48 edges, the partition rule and the placement rule separable",
        nodes,
        edges,
    }
}
