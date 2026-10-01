//! The fixture set both arms read: SciGraphs' own Les Miserables graph, the gate's model at
//! seeds 0..19, and one rooted tree, one DAG and one bipartite graph.
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

use crate::runner::workspace_root;
use graph_core::{
    EdgeKind, EdgeRecord, NodeKind, NodeRecord, REFERENCE_DEGREE, gate_node_count, seeded_model,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

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

/// SciGraphs' gallery graph: `fixtures/scigraphs/lesmis.json`, 77 nodes and 254 edges. The
/// one fixture whose node order is not ours to choose: `nodes[i].id` is the integer
/// SciGraphs' own run listed at position `i`, so position is preserved and only the *name*
/// changes to `n%04d`. A `nodes` array whose ids are not `0..n` is a hard error.
fn lesmis() -> Result<Fixture, String> {
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
fn tree() -> Result<Fixture, String> {
    from_repo(
        "fixtures/hierarchy/tree-balanced.json",
        "tree-balanced",
        "the repo's balanced binary tree, depth 3, 15 nodes, every parent-first spelling",
    )
}

/// One DAG: `fixtures/dag/diamond.json`, the smallest graph with a real layering choice.
fn dag() -> Result<Fixture, String> {
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
fn bipartite() -> Fixture {
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

/// The gate's own model at one seed, re-named dense so byte order is list order.
fn gate(seed: u32) -> Result<Fixture, String> {
    let count = gate_node_count(seed);
    if count > GATE_CAP {
        return Err(format!(
            "gate seed {seed} is {count} nodes, past the {GATE_CAP} cap"
        ));
    }
    let (mut nodes, mut edges) = seeded_model(seed, count, REFERENCE_DEGREE);
    let ids = names(nodes.len());
    // `seeded_model` names its nodes `n0..n{n}`, whose byte order is *not* its numeric order
    // past nine — so the ends are remapped through the original list rather than by string
    // surgery, and an end that is not a node is an error rather than a dangling edge.
    let original: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    for (record, id) in nodes.iter_mut().zip(&ids) {
        record.id = id.clone();
    }
    for (index, record) in edges.iter_mut().enumerate() {
        record.id = format!("e{index:05}");
        record.source = seat(&original, &ids, &record.source)?;
        record.target = seat(&original, &ids, &record.target)?;
    }
    Ok(Fixture {
        // Bounded by `GATE_SEEDS` and never by a caller's loop: 20 leaked names per emit.
        name: leak(format!("gate-{seed:02}")),
        about: leak(format!("the gate model at seed {seed}, {count} nodes")),
        nodes,
        edges,
    })
}

/// A `&'static str` out of a formatted value, because [`Fixture::name`] is `&'static str`
/// and a `Box::leak` is the honest way to say it: bounded by the seed list above.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

/// An edge end's new name, from the id the gate model gave it.
fn seat(original: &[String], ids: &[String], end: &str) -> Result<String, String> {
    original
        .iter()
        .position(|candidate| candidate == end)
        .map(|at| ids[at].clone())
        .ok_or_else(|| format!("edge end {end} is not a node of its fixture"))
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
