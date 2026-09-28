//! The layout oracle's fixtures: `harness/oracle-layouts.mjs` needs the same repaired
//! hierarchy every hierarchy layout lays out (to build a `d3.hierarchy` tree from) and
//! graph-core's own tidy-tree and treemap output (to byte-compare against), over the
//! same gate model `hashgate`/`roundtrip` use. Written alongside `emit-fixtures`'s own
//! files, into the same directory, under its own manifest: a different oracle (a
//! library, not `src/core/model`) with a different generator, kept out of the topology
//! manifest's shape rather than bolted onto it.
//!
//! One line of `layouts.jsonl` per seed:
//! ```json
//! {"seed": 7, "tree": {"id": 0, "weight": 0.42, "children": [...]},
//!  "tidy": {"x": [...], "y": [...]},
//!  "treemap": {"x": [...], "y": [...], "w": [...], "h": [...]}}
//! ```
//! `tree` is `d3.hierarchy`-shaped, children already in the ascending dense-index order
//! [`Hierarchy::children`] gives, so the oracle's `children(node)` callback is
//! `node => node.children`, no re-sorting. The virtual root (two or more real roots)
//! is `{"id": null, "weight": null, ...}`; a real node's `weight` is its topology weight,
//! unclamped — the oracle's own `clampWeight` (`layout::treemap`'s Ponytail) applies the
//! clamp, exactly as the module doc's call sequence has it.

use super::super::evidence::{FINGERPRINTED, Stamp};
use super::super::runner::file_sha256;
use graph_contract::geometry::NodeGeometry;
use graph_core::layout::hierarchy::Hierarchy;
use graph_core::layout::{tidy_tree, treemap};
use graph_core::{REFERENCE_DEGREE, StageError, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;

/// Writes `layouts.jsonl` and `layout-manifest.json` for seeds `0..seeds` into `out`.
pub fn write(seeds: u32, out: &Path, stamp: &Stamp) -> Result<(), String> {
    let path = out.join("layouts.jsonl");
    let mut file = super::create(&path)?;
    for seed in 0..seeds {
        let line = fixture_line(seed).map_err(|e| format!("seed {seed}: {e}"))?;
        writeln!(file, "{line}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    file.flush()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest = json!({
        "generator": "graph-cli emit-fixtures: the gate model, graph_core::seeded_model",
        "format": 1,
        "seeds": seeds,
        "sha256": { "layouts.jsonl": file_sha256(&path)? },
        "fingerprint": stamp.fingerprint(),
        "fingerprinted": FINGERPRINTED,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    stamp.still_current()?;
    let manifest_path = out.join("layout-manifest.json");
    std::fs::write(&manifest_path, text + "\n")
        .map_err(|e| format!("{}: {e}", manifest_path.display()))
}

/// One seed's line: the tree the oracle builds `d3.hierarchy` from, and graph-core's own
/// tidy-tree and treemap output over the same topology.
fn fixture_line(seed: u32) -> Result<Value, String> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let hierarchy = Hierarchy::of(&topology).map_err(|e| e.to_string())?;
    let root = hierarchy
        .root()
        .ok_or("empty topology: the gate model never draws one")?;
    let tree = tree_json(&hierarchy, &topology, root);
    let tidy = point_json(tidy_tree::run(&topology).map_err(stage_err)?)?;
    let treemap = box_json(treemap::run(&topology).map_err(stage_err)?)?;
    Ok(json!({ "seed": seed, "tree": tree, "tidy": tidy, "treemap": treemap }))
}

fn stage_err(err: StageError) -> String {
    err.to_string()
}

/// `v`'s node, recursively, children ascending dense index; the virtual root carries no
/// id and no weight, exactly as `d3.hierarchy`'s data node for it would not be a real
/// record.
fn tree_json(h: &Hierarchy, t: &graph_core::Topology, v: u32) -> Value {
    let children: Vec<Value> = h.children(v).iter().map(|&c| tree_json(h, t, c)).collect();
    if h.virtual_root() == Some(v) {
        json!({ "id": null, "weight": null, "children": children })
    } else {
        json!({ "id": v, "weight": t.node(v).weight, "children": children })
    }
}

fn point_json(geometry: graph_core::Geometry) -> Result<Value, String> {
    match geometry.nodes {
        NodeGeometry::Point { x, y } => Ok(json!({ "x": x, "y": y })),
        other => Err(format!("tidy tree emitted {other:?}, not Point")),
    }
}

fn box_json(geometry: graph_core::Geometry) -> Result<Value, String> {
    match geometry.nodes {
        NodeGeometry::Box { x, y, w, h } => Ok(json!({ "x": x, "y": y, "w": w, "h": h })),
        other => Err(format!("treemap emitted {other:?}, not Box")),
    }
}

#[cfg(test)]
mod tests;
