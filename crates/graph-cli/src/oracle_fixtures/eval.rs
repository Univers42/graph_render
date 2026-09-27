//! The Rust arm: each case read back from `cases.jsonl`, run through graph-core, and
//! written as the canonical JSON the TypeScript arm must reproduce byte for byte.

use super::wire::{WireEdge, WireLegend, WireModel, WireNode, WirePatch, hex};
use crate::runner::sha256_hex;
use graph_core as core;
use graph_core::{EdgeRecord, NodeKind, NodeRecord, Patch};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;

type Records = (Vec<NodeRecord>, Vec<EdgeRecord>);

/// Holds the graphs defined so far, for the cases that name them.
#[derive(Default)]
pub struct Evaluator {
    graphs: BTreeMap<String, Records>,
}

fn text<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args[key]
        .as_str()
        .ok_or_else(|| format!("argument {key} is not a string"))
}

/// Canonical JSON: compact, every object's keys in sorted order (serde_json's `Value`
/// map is a `BTreeMap` — the workspace never enables `preserve_order`, and the test
/// below fails if something does). The TypeScript arm sorts the same way, so field
/// order is not a thing the two arms can disagree on.
fn canonical(value: &impl Serialize) -> Result<String, String> {
    let value = serde_json::to_value(value).map_err(|e| e.to_string())?;
    serde_json::to_string(&value).map_err(|e| e.to_string())
}

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

impl Evaluator {
    /// The canonical output of `function` on `args`; `null` for a graph definition.
    pub fn eval(&mut self, function: &str, args: &Value) -> Result<String, String> {
        match function {
            "graph" => self.define(args),
            "makeRecordNodeId" | "makeNoteNodeId" | "makeTagNodeId" | "makeEdgeId" => {
                canonical(&ids(function, args)?)
            }
            "parseNodeId" => {
                let parsed = core::parse_node_id(text(args, "nodeId")?).map(|r| {
                    json!({ "source": r.source, "databaseId": r.database_id, "recordId": r.record_id })
                });
                canonical(&parsed)
            }
            "edgeKindFromType" => {
                canonical(&core::edge_kind_from_type(args["type"].as_str()).as_str())
            }
            "hashString" => canonical(&core::hash_string(text(args, "value")?)),
            "nodesEqual" => {
                let (a, b): (WireNode, WireNode) = (parse(&args["a"])?, parse(&args["b"])?);
                canonical(&core::nodes_equal(&a.record()?.view(), &b.record()?.view()))
            }
            "edgesEqual" => {
                let (a, b): (WireEdge, WireEdge) = (parse(&args["a"])?, parse(&args["b"])?);
                canonical(&core::edges_equal(&a.record()?.view(), &b.record()?.view()))
            }
            "isEmptyPatch" => canonical(&core::is_empty_patch(&patch_of(&args["lengths"])?)),
            "emptyModel" => canonical(&WireModel::of(&core::empty_model())),
            "buildSyntheticModel" => synthetic(args),
            "layoutGroups" => groups(args),
            _ => self.over_graphs(function, args),
        }
    }

    fn define(&mut self, args: &Value) -> Result<String, String> {
        let nodes: Vec<WireNode> = parse(&args["nodes"])?;
        let edges: Vec<WireEdge> = parse(&args["edges"])?;
        let nodes = nodes
            .iter()
            .map(WireNode::record)
            .collect::<Result<_, _>>()?;
        let edges = edges
            .iter()
            .map(WireEdge::record)
            .collect::<Result<_, _>>()?;
        self.graphs
            .insert(text(args, "name")?.to_owned(), (nodes, edges));
        Ok("null".into())
    }

    fn graph(&self, args: &Value, key: &str) -> Result<&Records, String> {
        let name = text(args, key)?;
        self.graphs
            .get(name)
            .ok_or_else(|| format!("graph {name} is not defined"))
    }

    fn indexed(&self, args: &Value, key: &str) -> Result<core::Topology, String> {
        let (nodes, edges) = self.graph(args, key)?;
        core::index_model(nodes, edges).map_err(|e| e.to_string())
    }

    fn over_graphs(&self, function: &str, args: &Value) -> Result<String, String> {
        match function {
            "applyDegreeWeights" => {
                let (nodes, edges) = self.graph(args, "graph")?;
                let mut nodes = nodes.clone();
                core::apply_degree_weights(&mut nodes, edges);
                canonical(&nodes.iter().map(|n| hex(n.weight)).collect::<Vec<_>>())
            }
            "indexModel" => canonical(&WireModel::of(&self.indexed(args, "graph")?)),
            "diffGraph" => {
                let (p, n) = (self.indexed(args, "previous")?, self.indexed(args, "next")?);
                canonical(&WirePatch::of(&core::diff_graph(&p, &n), &p, &n))
            }
            "deriveLegend" => {
                let t = self.indexed(args, "graph")?;
                canonical(&WireLegend::of(&core::derive_legend(&t), &t))
            }
            "neighborhood" | "neighborhoodEdges" => self.hood(function, args),
            other => Err(format!("unknown function {other}")),
        }
    }

    fn hood(&self, function: &str, args: &Value) -> Result<String, String> {
        let t = self.indexed(args, "graph")?;
        let depth = args["depth"].as_u64().and_then(|d| u32::try_from(d).ok());
        let depth = depth.ok_or("depth is not a u32")?;
        let hood = core::neighborhood_edges(&t, text(args, "id")?, depth);
        let node_ids: Vec<&str> = hood.nodes.iter().map(|&v| t.node(v).id).collect();
        if function == "neighborhood" {
            return canonical(&node_ids);
        }
        let edge_ids: Vec<&str> = hood.edges.iter().map(|&e| t.edge(e).id).collect();
        canonical(&json!({ "nodeIds": node_ids, "edgeIds": edge_ids }))
    }
}

fn ids(function: &str, args: &Value) -> Result<String, String> {
    Ok(match function {
        "makeRecordNodeId" => core::make_record_node_id(
            text(args, "source")?,
            text(args, "databaseId")?,
            text(args, "recordId")?,
        ),
        "makeNoteNodeId" => core::make_note_node_id(text(args, "noteId")?),
        "makeTagNodeId" => core::make_tag_node_id(text(args, "tagValue")?),
        _ => {
            let kind = text(args, "kind")?;
            let kind = core::EdgeKind::from_name(kind).ok_or(format!("edge kind {kind}"))?;
            let directed = args["directed"].as_bool().ok_or("directed is not a bool")?;
            let (s, t, label) = (
                text(args, "source")?,
                text(args, "target")?,
                text(args, "label")?,
            );
            core::make_edge_id(s, t, kind, label, directed)
        }
    })
}

fn patch_of(lengths: &Value) -> Result<Patch, String> {
    let lengths: Vec<usize> = parse(lengths)?;
    let [a, b, c, d, e, f] = lengths[..] else {
        return Err("isEmptyPatch takes six lengths".into());
    };
    Ok(Patch {
        added_nodes: vec![0; a],
        updated_nodes: vec![0; b],
        removed_nodes: vec![0; c],
        added_edges: vec![0; d],
        updated_edges: vec![0; e],
        removed_edges: vec![0; f],
    })
}

fn synthetic(args: &Value) -> Result<String, String> {
    let bits = u64::from_str_radix(text(args, "n")?, 16).map_err(|e| e.to_string())?;
    let model = core::build_synthetic_model(f64::from_bits(bits)).map_err(|e| e.to_string())?;
    let text = canonical(&WireModel::of(&model))?;
    if args["digest"].as_bool() == Some(true) {
        return canonical(&json!({ "sha256": sha256_hex(text.as_bytes()) }));
    }
    Ok(text)
}

/// The H9 arm: the group column of nodes that carry only an id and a source.
fn groups(args: &Value) -> Result<String, String> {
    let nodes: Vec<NodeRecord> = args["nodes"]
        .as_array()
        .ok_or("nodes is not an array")?
        .iter()
        .map(|n| {
            Ok(NodeRecord {
                id: text(n, "id")?.to_owned(),
                kind: NodeKind::Record,
                database_id: None,
                source: text(n, "source")?.to_owned(),
                label: String::new(),
                group: None,
                weight: 0.5,
                version: 0.0,
                has_note: false,
                icon: None,
            })
        })
        .collect::<Result<_, String>>()?;
    let t = core::index_model(&nodes, &[]).map_err(|e| e.to_string())?;
    canonical(&t.nodes().group)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_is_compact_with_sorted_keys_at_every_depth() {
        let nested = WireNode {
            id: "a".into(),
            kind: "record".into(),
            database_id: None,
            source: "pg".into(),
            label: String::new(),
            group: None,
            weight: hex(0.5),
            version: hex(0.0),
            has_note: false,
            icon: None,
        };
        let text = canonical(&json!({ "z": [nested], "a": 1 })).expect("serialises");
        let want = r#"{"a":1,"z":[{"databaseId":null,"group":null,"hasNote":false,"icon":null,"id":"a","kind":"record","label":"","source":"pg","version":"0000000000000000","weight":"3fe0000000000000"}]}"#;
        assert_eq!(text, want);
    }

    #[test]
    fn a_graph_definition_is_null_and_unknown_functions_are_refused() {
        let mut evaluator = Evaluator::default();
        let graph = json!({ "name": "g", "nodes": [], "edges": [] });
        assert_eq!(evaluator.eval("graph", &graph), Ok("null".into()));
        assert_eq!(
            evaluator
                .eval("indexModel", &json!({ "graph": "g" }))
                .map(|s| s.len() > 2),
            Ok(true)
        );
        assert!(
            evaluator
                .eval("indexModel", &json!({ "graph": "h" }))
                .is_err()
        );
        assert!(evaluator.eval("layoutTree", &json!({})).is_err());
    }
}
