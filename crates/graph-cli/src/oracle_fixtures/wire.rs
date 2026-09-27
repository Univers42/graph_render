//! The fixture wire format — one shape per oracle type, with the oracle's field names,
//! written as canonical JSON (sorted keys, see `eval.rs`) so `serde_json` here and
//! `JSON.stringify` in `harness/oracle-diff.mjs` write the same bytes. Every `number`
//! that is not a count travels as the 16 hex digits of its IEEE-754 bits: JSON cannot
//! carry `NaN`, `-0` or ±∞, and a decimal round trip is one more thing that could differ.

use graph_core::{
    EdgeKind, EdgeRecord, EdgeView, LegendCounts, NodeKind, NodeRecord, NodeView, Patch, Topology,
};
use serde::{Deserialize, Serialize};

/// `f64` as its bits, 16 lowercase hex digits.
pub fn hex(value: f64) -> String {
    format!("{:016x}", value.to_bits())
}

fn unhex(text: &str) -> Result<f64, String> {
    if text.len() != 16 {
        return Err(format!("bad f64 bits {text:?}"));
    }
    u64::from_str_radix(text, 16)
        .map(f64::from_bits)
        .map_err(|e| format!("bad f64 bits {text:?}: {e}"))
}

/// `GraphNode`, minus the lazy `fields`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireNode {
    pub id: String,
    pub kind: String,
    pub database_id: Option<String>,
    pub source: String,
    pub label: String,
    pub group: Option<String>,
    pub weight: String,
    pub version: String,
    pub has_note: bool,
    pub icon: Option<String>,
}

/// `GraphEdge`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WireEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub label: String,
    pub strength: String,
    pub directed: bool,
    pub record_id: Option<String>,
}

impl WireNode {
    pub fn of(view: &NodeView<'_>) -> Self {
        Self {
            id: view.id.into(),
            kind: view.kind.as_str().into(),
            database_id: view.database_id.map(Into::into),
            source: view.source.into(),
            label: view.label.into(),
            group: view.group.map(Into::into),
            weight: hex(view.weight),
            version: hex(view.version),
            has_note: view.has_note,
            icon: view.icon.map(Into::into),
        }
    }

    pub fn record(&self) -> Result<NodeRecord, String> {
        Ok(NodeRecord {
            id: self.id.clone(),
            kind: NodeKind::from_name(&self.kind).ok_or(format!("node kind {:?}", self.kind))?,
            database_id: self.database_id.clone(),
            source: self.source.clone(),
            label: self.label.clone(),
            group: self.group.clone(),
            weight: unhex(&self.weight)?,
            version: unhex(&self.version)?,
            has_note: self.has_note,
            icon: self.icon.clone(),
        })
    }
}

impl WireEdge {
    pub fn of(view: &EdgeView<'_>) -> Self {
        Self {
            id: view.id.into(),
            source: view.source.into(),
            target: view.target.into(),
            kind: view.kind.as_str().into(),
            label: view.label.into(),
            strength: hex(view.strength),
            directed: view.directed,
            record_id: view.record_id.map(Into::into),
        }
    }

    pub fn record(&self) -> Result<EdgeRecord, String> {
        Ok(EdgeRecord {
            id: self.id.clone(),
            source: self.source.clone(),
            target: self.target.clone(),
            kind: EdgeKind::from_name(&self.kind).ok_or(format!("edge kind {:?}", self.kind))?,
            label: self.label.clone(),
            strength: unhex(&self.strength)?,
            directed: self.directed,
            record_id: self.record_id.clone(),
        })
    }
}

/// `GraphStats`.
#[derive(Serialize)]
pub struct WireStats {
    nodes: u32,
    edges: u32,
    databases: u32,
    notes: u32,
}

/// `GraphModel`. `nodeById`/`edgeById` are `nodes`/`edges` keyed by id, in the same
/// order, so they add nothing; the two `Map`s that do are written as `[key, values]`
/// pairs in the `Map`'s own iteration order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WireModel {
    nodes: Vec<WireNode>,
    edges: Vec<WireEdge>,
    adjacency: Vec<(String, Vec<String>)>,
    by_database: Vec<(String, Vec<String>)>,
    stats: WireStats,
}

impl WireModel {
    pub fn of(t: &Topology) -> Self {
        let node_id = |v: u32| t.node(v).id.to_owned();
        let edge_ids = |v: u32| t.incident(v).map(|e| t.edge(e).id.to_owned()).collect();
        let stats = t.stats();
        Self {
            nodes: (0..t.node_count())
                .map(|i| WireNode::of(&t.node(i)))
                .collect(),
            edges: (0..t.edge_count())
                .map(|e| WireEdge::of(&t.edge(e)))
                .collect(),
            adjacency: first_touch(t)
                .into_iter()
                .map(|v| (node_id(v), edge_ids(v)))
                .collect(),
            by_database: t
                .by_database()
                .map(|(db, nodes)| (db.to_owned(), nodes.iter().map(|&v| node_id(v)).collect()))
                .collect(),
            stats: WireStats {
                nodes: stats.nodes,
                edges: stats.edges,
                databases: stats.databases,
                notes: stats.notes,
            },
        }
    }
}

/// Nodes in the order the oracle's `adjacency` `Map` first receives them: edge by edge,
/// source before target (`model.ts:51-52`). Nodes without edges are absent, as there.
fn first_touch(t: &Topology) -> Vec<u32> {
    let mut seen = vec![false; t.node_count() as usize];
    let mut order = Vec::new();
    let columns = t.edges();
    for (&source, &target) in columns.source.iter().zip(&columns.target) {
        for v in [source, target] {
            if !std::mem::replace(&mut seen[v as usize], true) {
                order.push(v);
            }
        }
    }
    order
}

/// `GraphPatch`, nodes and edges written out from the side each list indexes.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WirePatch {
    added_nodes: Vec<WireNode>,
    updated_nodes: Vec<WireNode>,
    removed_node_ids: Vec<String>,
    added_edges: Vec<WireEdge>,
    updated_edges: Vec<WireEdge>,
    removed_edge_ids: Vec<String>,
}

impl WirePatch {
    pub fn of(patch: &Patch, previous: &Topology, next: &Topology) -> Self {
        let nodes = |list: &[u32]| list.iter().map(|&i| WireNode::of(&next.node(i))).collect();
        let edges = |list: &[u32]| list.iter().map(|&i| WireEdge::of(&next.edge(i))).collect();
        Self {
            added_nodes: nodes(&patch.added_nodes),
            updated_nodes: nodes(&patch.updated_nodes),
            removed_node_ids: patch
                .removed_nodes
                .iter()
                .map(|&i| previous.node(i).id.into())
                .collect(),
            added_edges: edges(&patch.added_edges),
            updated_edges: edges(&patch.updated_edges),
            removed_edge_ids: patch
                .removed_edges
                .iter()
                .map(|&i| previous.edge(i).id.into())
                .collect(),
        }
    }
}

/// `Legend` projected to counts: `[id, label, count]` per database and tag, `[kind,
/// count]` per kind. The oracle's `color` is dropped — colour stays in TypeScript (H7).
#[derive(Serialize)]
pub struct WireLegend {
    databases: Vec<(String, String, u32)>,
    tags: Vec<(String, String, u32)>,
    kinds: Vec<(String, u32)>,
}

impl WireLegend {
    pub fn of(legend: &LegendCounts<'_>, t: &Topology) -> Self {
        Self {
            databases: legend
                .databases
                .iter()
                .map(|d| (d.id.into(), d.id.into(), d.count))
                .collect(),
            tags: legend
                .tags
                .iter()
                .map(|tag| {
                    (
                        t.node(tag.node).id.into(),
                        t.node(tag.node).label.into(),
                        tag.count,
                    )
                })
                .collect(),
            kinds: legend
                .kinds
                .iter()
                .map(|&(k, n)| (k.as_str().into(), n))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_is_the_bit_pattern_and_round_trips_every_special_value() {
        assert_eq!(hex(1.0), "3ff0000000000000");
        assert_eq!(hex(-0.0), "8000000000000000");
        for value in [0.0, -0.0, f64::INFINITY, f64::MIN_POSITIVE, 0.1] {
            assert_eq!(unhex(&hex(value)).map(f64::to_bits), Ok(value.to_bits()));
        }
        assert!(unhex(&hex(f64::NAN)).expect("parses").is_nan());
        assert!(unhex("3ff").is_err() && unhex("3ff000000000000g").is_err());
    }
}
