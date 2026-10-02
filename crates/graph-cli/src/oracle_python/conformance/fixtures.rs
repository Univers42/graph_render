//! The fixture set both arms read: SciGraphs' own Les Miserables graph, the gate's model at
//! seeds 0..19, and one rooted tree, one DAG and one bipartite graph.
//!
//! **The set is built in child modules, this file is the spine they share.** `named.rs` holds
//! the four fixtures read off `fixtures/*.json` (`lesmis`, `tree`, `dag`, `bipartite`) and
//! `gate_model.rs` the twenty generated gate seeds. What stays here is what all of them need:
//! the [`Fixture`] struct, the dense node naming both arms read, the repo-fixture reader and
//! the one line both arms are handed.
//!
//! **Node order is the whole contract of this file.** SciGraphs' graph is a plain dict, so its
//! node order is whatever the dict listed; the motor's is the topology's insertion order. The
//! rule the fixture encodes: *SciGraphs node `i` is the motor node whose id sorts `i`-th in
//! byte order.* Every fixture here names its nodes `n%0Nd` in the order it lists them, which
//! makes byte order and list order the same thing **by construction** — and
//! `harness/scigraphs-conformance.py` sorts the ids anyway and refuses on a disagreement, so
//! the rule is checked rather than assumed.
//!
//! **The gate model is the graph every other oracle in this repo measures over**, so these
//! numbers are comparable with `docs/measurements/scigraphs-coverage.md`. Seeds 0..19 give
//! 2..21 nodes (`gate_node_count`, `stage/topology.rs:19-21`), so the 200-node cap is
//! reached by none of them; a seed that would reach it is refused, not clipped.

mod gate_model;
mod named;

use crate::runner::workspace_root;
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

// Re-exported so the tests, and nothing else, name a fixture through this module.
use gate_model::gate;
use named::{bipartite, dag, lesmis, tree};

/// The gate model seeds 0..19, as the job asks.
pub const GATE_SEEDS: u32 = 20;

/// The node-count cap the job states for the gate model. No seed in [`GATE_SEEDS`] reaches it
/// (`gate_node_count` gives at most 21), so nothing is truncated.
pub const GATE_CAP: u32 = 200;

/// One fixture: a name, the dense node order both arms use, and the edges as dense indices
/// into it.
pub struct Fixture {
    pub name: &'static str,
    pub about: &'static str,
    pub nodes: Vec<NodeRecord>,
    pub edges: Vec<EdgeRecord>,
}

/// The whole set, in the order both arms walk it. The order is fixed, so a row's coordinate
/// count is a function of the tree and not of iteration order.
pub fn all() -> Result<Vec<Fixture>, String> {
    let mut out = vec![lesmis()?, tree()?, dag()?, bipartite()];
    for seed in 0..GATE_SEEDS {
        out.push(gate(seed)?);
    }
    Ok(out)
}

/// One `record` node. Only the id and the weight are load-bearing: `apply_degree_weights`
/// overwrites the weight from the graph's own degrees, and no layout in this matrix reads a
/// label.
fn node(id: &str) -> NodeRecord {
    NodeRecord {
        id: id.to_string(),
        kind: NodeKind::Record,
        database_id: None,
        source: "conformance".to_string(),
        label: id.to_string(),
        group: None,
        weight: 1.0,
        version: 1.0,
        has_note: false,
        icon: None,
    }
}

/// One edge, `Relation`, undirected. Ids are `e00000..` in fixture order, so an edge's id is a
/// function of its index and the arms never have to agree on how to name one.
fn edge(index: usize, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord {
        id: format!("e{index:05}"),
        source: source.to_string(),
        target: target.to_string(),
        kind: EdgeKind::Relation,
        label: String::new(),
        strength: 1.0,
        directed: false,
        record_id: None,
        child_first: false,
    }
}

/// The node names for `count` dense indices, zero-padded to the width of `count` itself, so
/// byte order and numeric order agree — the property the mapping needs and the fixture's
/// only test that would catch a regression of it.
fn names(count: usize) -> Vec<String> {
    let width = count.to_string().len().max(1);
    (0..count).map(|i| format!("n{i:0width$}")).collect()
}

/// A repo fixture in the repo's own `nodes`/`edges` shape, with its ids re-listed dense.
fn from_repo(relative: &str, name: &'static str, about: &'static str) -> Result<Fixture, String> {
    let path = workspace_root().join(relative);
    let doc = read(&path)?;
    let listed: Vec<String> = array(&doc, "nodes", &path)?
        .iter()
        .map(|n| {
            n["id"]
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{}: a node has no string id", path.display()))
        })
        .collect::<Result<_, _>>()?;
    // The repo lists `r` before `a`, so its order is **not** byte order — which is why the
    // fixture states a mapping. Both arms read the byte-sorted list, so SciGraphs' index `i`
    // is the motor id that sorts `i`-th, and the tree's edges keep their meaning because they
    // name ids rather than positions.
    let mut listed = listed;
    listed.sort();
    let nodes: Vec<NodeRecord> = listed.iter().map(|id| node(id)).collect();
    let seat = |id: &str| -> Result<usize, String> {
        listed
            .iter()
            .position(|candidate| candidate == id)
            .ok_or_else(|| format!("{}: edge end {id} is not a node", path.display()))
    };
    let mut edges = Vec::new();
    for entry in array(&doc, "edges", &path)? {
        let ends = [
            entry["source"].as_str().ok_or("no source")?,
            entry["target"].as_str().ok_or("no target")?,
        ];
        edges.push(edge(
            edges.len(),
            &listed[seat(ends[0])?],
            &listed[seat(ends[1])?],
        ));
    }
    Ok(Fixture {
        name,
        about,
        nodes,
        edges,
    })
}

fn read(path: &PathBuf) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn array<'a>(doc: &'a Value, key: &str, path: &Path) -> Result<&'a Vec<Value>, String> {
    doc.get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{}: no {key} array", path.display()))
}

/// The fixture line both arms read: the dense node order, the mapping stated explicitly, and
/// the edges as dense indices into that order.
pub fn line(fixture: &Fixture) -> Value {
    let ids: Vec<&str> = fixture.nodes.iter().map(|n| n.id.as_str()).collect();
    // `u32::MAX` is the fallback and is refused downstream: a dangling endpoint must not
    // be able to index a node.
    let seat = |id: &str| -> u32 { ids.iter().position(|c| *c == id).unwrap_or(usize::MAX) as u32 };
    let mapping: Vec<Value> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| json!([i, id]))
        .collect();
    json!({
        "name": fixture.name,
        "about": fixture.about,
        "n": fixture.nodes.len(),
        "nodes": ids,
        "mapping": mapping,
        "source": fixture.edges.iter().map(|e| seat(&e.source)).collect::<Vec<u32>>(),
        "target": fixture.edges.iter().map(|e| seat(&e.target)).collect::<Vec<u32>>(),
    })
}

#[cfg(test)]
mod tests;
