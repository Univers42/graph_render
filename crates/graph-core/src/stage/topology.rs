//! The topology stage's input and bytes: a synthetic model, remixed by seed so every part
//! of the topology is populated, and every column, CSR and string of the indexed result
//! written out — so native and wasm32 are compared on everything the topology holds, not
//! only on what the oracle's benchmark graph happens to exercise.

use super::StageError;
use crate::edgekind::EdgeKind;
use crate::index::Topology;
use crate::records::{EdgeRecord, NodeRecord};
use crate::synthetic::{Mulberry32, synthetic_records};
use crate::weights::apply_degree_weights_against;

/// Distinct sources the remix draws from: more than 256, so the group column crosses
/// the oracle's `Uint8Array` limit (H9) on every large enough seed.
const SOURCES: usize = 300;

/// Nodes the gate's model has for `seed`: `2 + seed % 600`, so the sweep covers every
/// size from 2 to 601.
pub const fn gate_node_count(seed: u32) -> u32 {
    2 + seed % 600
}

/// The records of the `count`-node synthetic model, sources, edge kinds and hierarchy
/// orientation redrawn from `seed`, weights against `reference_degree`: the pipeline's
/// input for one seed.
pub fn seeded_model(
    seed: u32,
    count: u32,
    reference_degree: u32,
) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let (mut nodes, mut edges) = synthetic_records(count);
    remix(seed, &mut nodes, &mut edges);
    apply_degree_weights_against(&mut nodes, &edges, reference_degree);
    (nodes, edges)
}

fn remix(seed: u32, nodes: &mut [NodeRecord], edges: &mut [EdgeRecord]) {
    let mut rnd = Mulberry32::new(seed);
    for node in nodes {
        node.source = format!("s{}", rnd.pick(SOURCES));
    }
    for edge in edges {
        edge.kind = EdgeKind::ALL[rnd.pick(EdgeKind::ALL.len())];
        edge.child_first = edge.kind == EdgeKind::Hierarchy && rnd.pick(2) == 1;
    }
}

/// Every column, CSR and string of `t`, in a fixed little-endian layout, so a divergence
/// anywhere in the topology changes the bytes.
pub(super) fn encode(t: &Topology, out: &mut Vec<u8>) -> Result<(), StageError> {
    let stats = t.stats();
    for count in [stats.nodes, stats.edges, stats.databases, stats.notes] {
        put_u32(out, count);
    }
    (0..t.node_count()).try_for_each(|i| encode_node(t, i, out))?;
    (0..t.edge_count()).try_for_each(|e| encode_edge(t, e, out))?;
    for csr in [t.out(), t.inbound(), t.hierarchy()] {
        (0..csr.rows()).for_each(|r| put_u32s(out, csr.row(r)));
    }
    for (database, members) in t.by_database() {
        put_str(out, database);
        put_u32s(out, members);
    }
    Ok(())
}

fn encode_node(t: &Topology, i: u32, out: &mut Vec<u8>) -> Result<(), StageError> {
    let node = t.node(i);
    put_str(out, node.id);
    out.push(node.kind as u8);
    for text in [
        node.database_id,
        Some(node.source),
        Some(node.label),
        node.group,
        node.icon,
    ] {
        put_opt(out, text);
    }
    put_f64(out, node.weight, "weight")?;
    put_f64(out, node.version, "version")?;
    out.push(u8::from(node.has_note));
    put_u32(out, t.nodes().group[i as usize]);
    put_u32(out, t.nodes().degree[i as usize]);
    Ok(())
}

fn encode_edge(t: &Topology, e: u32, out: &mut Vec<u8>) -> Result<(), StageError> {
    let (edge, columns) = (t.edge(e), t.edges());
    put_str(out, edge.id);
    put_u32(out, columns.source[e as usize]);
    put_u32(out, columns.target[e as usize]);
    out.extend([
        edge.kind as u8,
        u8::from(edge.directed),
        u8::from(edge.child_first),
    ]);
    put_str(out, edge.label);
    put_opt(out, edge.record_id);
    put_f64(out, edge.strength, "strength")
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32s(out: &mut Vec<u8>, values: &[u32]) {
    put_u32(out, values.len() as u32);
    values.iter().for_each(|&v| put_u32(out, v));
}

fn put_str(out: &mut Vec<u8>, text: &str) {
    put_u32(out, text.len() as u32);
    out.extend_from_slice(text.as_bytes());
}

fn put_opt(out: &mut Vec<u8>, text: Option<&str>) {
    match text {
        None => out.push(0),
        Some(text) => {
            out.push(1);
            put_str(out, text);
        }
    }
}

fn put_f64(out: &mut Vec<u8>, value: f64, column: &'static str) -> Result<(), StageError> {
    if !value.is_finite() {
        return Err(StageError::NonFinite { column });
    }
    out.extend_from_slice(&value.to_bits().to_le_bytes());
    Ok(())
}

#[cfg(test)]
mod tests;
