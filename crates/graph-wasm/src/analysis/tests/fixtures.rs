//! Test fixtures for analysis tests.

use crate::ingest;
use graph_core::index_model;
use graph_core::Topology;

pub fn node_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

pub fn edge_json(id: &str, source: &str, target: &str) -> String {
    directed_edge_json(id, source, target, false)
}

/// The same edge, direction respected or not. A `relation` edge is undirected unless the
/// document says `directed: true`, so the two kinds of component can only be told apart
/// on a fixture that actually sets it.
pub fn directed_edge_json(id: &str, source: &str, target: &str, directed: bool) -> String {
    format!(
        r#"{{"id":"{id}","source":"{source}","target":"{target}","kind":"relation","label":"","strength":0.5,"directed":{directed},"record_id":null}}"#
    )
}

/// A topology through the same reader `gm_build` uses, so these tests need no second
/// way of making a graph.
pub fn topology(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<String> = node_ids.iter().map(|id| node_json(id)).collect();
    let edges: Vec<String> = edges.iter().map(|(id, s, t)| edge_json(id, s, t)).collect();
    indexed(&nodes, &edges)
}

/// The same, with every edge directed.
pub fn directed(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<String> = node_ids.iter().map(|id| node_json(id)).collect();
    let edges: Vec<String> = edges
        .iter()
        .map(|(id, s, t)| directed_edge_json(id, s, t, true))
        .collect();
    indexed(&nodes, &edges)
}

pub fn indexed(nodes: &[String], edges: &[String]) -> Topology {
    let text = format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    );
    let (nodes, edges) = ingest::read(text.as_bytes()).expect("the fixture is valid");
    index_model(&nodes, &edges).expect("the fixture indexes")
}

/// A path `a - b - c`: `b` is an articulation point, so components, communities and
/// depth all have something to say, and every value below is derivable by hand.
pub fn path() -> Topology {
    topology(&["a", "b", "c"], &[("e0", "a", "b"), ("e1", "b", "c")])
}

/// The two directed edges `a -> b` and `b -> a`: the only shape on which strong
/// components can differ from weak ones.
pub fn two_cycle() -> Topology {
    directed(&["a", "b"], &[("e0", "a", "b"), ("e1", "b", "a")])
}

/// A single directed `a -> b` and nothing else: weak joins the two, strong does not, and
/// the hierarchy is empty so both nodes are roots.
pub fn directed_one_way() -> Topology {
    directed(&["a", "b"], &[("e0", "a", "b")])
}

/// A parent-child edge, which is what the hierarchy CSR reads — `kind: "hierarchy"`
/// with the parent as source (D-Q1: only `child_of` is flipped).
pub fn tree_json(id: &str, parent: &str, child: &str) -> String {
    format!(
        r#"{{"id":"{id}","source":"{parent}","target":"{child}","kind":"hierarchy","label":"parent_of","strength":0.5,"directed":true,"record_id":null}}"#
    )
}

/// `a` above `b` above `c`, plus an isolated `z`: the two-roots case, where every real
/// root hangs off the virtual root at dense index `n` and so sits at depth 1 (D-H).
pub fn forest() -> Topology {
    let nodes: Vec<String> = ["a", "b", "c", "z"]
        .iter()
        .map(|id| node_json(id))
        .collect();
    let edges = vec![tree_json("t0", "a", "b"), tree_json("t1", "b", "c")];
    indexed(&nodes, &edges)
}

/// The same chain without the isolated node, so there is exactly one root.
pub fn forest_without_the_isolated_node() -> Topology {
    let nodes: Vec<String> = ["a", "b", "c"].iter().map(|id| node_json(id)).collect();
    let edges = vec![tree_json("t0", "a", "b"), tree_json("t1", "b", "c")];
    indexed(&nodes, &edges)
}

pub fn ids() -> Vec<&'static str> {
    crate::analysis::registry::ANALYSES.iter().map(|entry| entry.id).collect()
}

pub fn index_of(id: &str) -> u32 {
    ids()
        .iter()
        .position(|&candidate| candidate == id)
        .map(|i| u32::try_from(i).expect("small"))
        .unwrap_or_else(|| panic!("{id} is not registered"))
}

/// The number of elements the JSON's `nodeCount` member says, or `None`.
pub fn count_of(value: &graph_contract::canonical_json::Value) -> Option<u64> {
    let graph_contract::canonical_json::Value::Object(members) = value else {
        return None;
    };
    members
        .iter()
        .find(|(k, _)| k == "nodeCount")
        .and_then(|(_, v)| match v {
            graph_contract::canonical_json::Value::Number(text) => text.parse().ok(),
            _ => None,
        })
}