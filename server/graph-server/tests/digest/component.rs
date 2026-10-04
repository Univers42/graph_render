//! The connected components of a studio fixture, so the manifest can be checked for one in
//! spectral's LOBPCG window (257–700 nodes, condition 7).
//!
//! Components are taken over the ingest document's own endpoints, undirected and direction
//! ignored: an LOBPCG branch is found per weakly connected block, so `directed` is not read.
//! Caveat: this counts what the document says, before `ingest::index` drops a duplicate id or
//! an edge with a missing endpoint, so a component here can be larger than the one the motor
//! sees. It is a floor on the window, never a substitute for the layout's own branch choice.

use serde_json::Value;
use std::collections::BTreeMap;

/// The node count of the largest weakly connected component of a studio ingest document, or a
/// refusal naming what could not be read.
pub fn largest_component(bytes: &[u8]) -> Result<usize, String> {
    let value: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("not JSON: {error}"))?;
    let nodes = value["nodes"].as_array().ok_or("no `nodes` array")?;
    let dense = dense_index(nodes)?;
    let mut forest = Forest::new(dense.len());
    for edge in value["edges"].as_array().ok_or("no `edges` array")? {
        let (Some(from), Some(to)) = (
            edge["source"].as_str().and_then(|id| dense.get(id)),
            edge["target"].as_str().and_then(|id| dense.get(id)),
        ) else {
            continue;
        };
        forest.union(*from, *to);
    }
    Ok(forest.largest())
}

/// `node id -> dense index`, in document order, so the components are numbered the way the
/// motor numbers its nodes rather than by any map's iteration order.
fn dense_index(nodes: &[Value]) -> Result<BTreeMap<&str, usize>, String> {
    nodes
        .iter()
        .enumerate()
        .map(|(at, node)| {
            let id = node["id"]
                .as_str()
                .ok_or_else(|| format!("node {} has no `id` string", at + 1))?;
            Ok((id, at))
        })
        .collect()
}

/// A union-find over `len` slots: `root` walks to the representative, `union` joins two.
struct Forest {
    parent: Vec<usize>,
}

impl Forest {
    /// `len` singleton components.
    fn new(len: usize) -> Self {
        Self {
            parent: (0..len).collect(),
        }
    }

    /// The representative of `slot`, with the path halved on the way.
    fn root(&mut self, mut slot: usize) -> usize {
        while self.parent[slot] != slot {
            self.parent[slot] = self.parent[self.parent[slot]];
            slot = self.parent[slot];
        }
        slot
    }

    /// Joins the components of `a` and `b`.
    fn union(&mut self, a: usize, b: usize) {
        let (a, b) = (self.root(a), self.root(b));
        if a != b {
            self.parent[a] = b;
        }
    }

    /// The node count of the largest component.
    fn largest(&mut self) -> usize {
        let mut sizes: BTreeMap<usize, usize> = BTreeMap::new();
        for slot in 0..self.parent.len() {
            *sizes.entry(self.root(slot)).or_default() += 1;
        }
        sizes.values().copied().max().unwrap_or(0)
    }
}
