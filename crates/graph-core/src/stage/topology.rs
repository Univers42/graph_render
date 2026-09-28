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
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};
    use crate::weights::REFERENCE_DEGREE;

    fn topology_stage(seed: u32, reference_degree: u32) -> Result<Vec<u8>, StageError> {
        let (nodes, edges) = seeded_model(seed, gate_node_count(seed), reference_degree);
        let mut out = Vec::new();
        encode(
            &index_model(&nodes, &edges).map_err(StageError::Capacity)?,
            &mut out,
        )?;
        Ok(out)
    }

    #[test]
    fn the_stage_is_deterministic_and_seed_and_reference_reach_it() {
        let a = topology_stage(5, REFERENCE_DEGREE).expect("fits");
        assert!(!a.is_empty());
        assert_eq!(a, topology_stage(5, REFERENCE_DEGREE).expect("fits"));
        assert_ne!(a, topology_stage(6, REFERENCE_DEGREE).expect("fits"));
        assert_ne!(a, topology_stage(5, REFERENCE_DEGREE + 1).expect("fits"));
    }

    #[test]
    fn a_non_finite_float_is_refused_not_hashed() {
        let mut nan = node("a", "");
        nan.weight = f64::NAN;
        let mut out = Vec::new();
        let topology = index_model(&[nan], &[]).expect("fits");
        let err = encode(&topology, &mut out).expect_err("NaN weight");
        assert_eq!(err, StageError::NonFinite { column: "weight" });
        assert_eq!(err.to_string(), "non-finite value in column weight");
        let mut far = edge("e", "a", "a");
        far.strength = f64::INFINITY;
        let topology = index_model(&[node("a", "")], &[far]).expect("fits");
        let err = encode(&topology, &mut Vec::new()).expect_err("infinite strength");
        assert_eq!(err, StageError::NonFinite { column: "strength" });
    }

    #[test]
    fn the_remix_populates_every_edge_kind_and_crosses_256_groups() {
        let (mut nodes, mut edges) = synthetic_records(600);
        remix(599, &mut nodes, &mut edges);
        let topology = index_model(&nodes, &edges).expect("fits");
        assert!(topology.nodes().group.iter().any(|&g| g > 255));
        for kind in EdgeKind::ALL {
            assert!(edges.iter().any(|e| e.kind == kind), "{kind:?}");
        }
        assert!(!topology.hierarchy().is_empty());
        let hierarchy = || edges.iter().filter(|e| e.kind == EdgeKind::Hierarchy);
        assert!(hierarchy().any(|e| e.child_first) && hierarchy().any(|e| !e.child_first));
        assert!(
            edges
                .iter()
                .all(|e| !e.child_first || e.kind == EdgeKind::Hierarchy)
        );
    }

    #[test]
    fn the_child_first_flag_reaches_the_bytes() {
        let nodes = [node("a", ""), node("b", "")];
        let mut parent_of = edge("h", "a", "b");
        parent_of.kind = EdgeKind::Hierarchy;
        let child_of = EdgeRecord {
            child_first: true,
            ..parent_of.clone()
        };
        let bytes = |e: EdgeRecord| {
            let mut out = Vec::new();
            encode(&index_model(&nodes, &[e]).expect("fits"), &mut out).expect("finite");
            out
        };
        assert_ne!(bytes(parent_of), bytes(child_of));
    }

    #[test]
    fn the_layout_is_pinned_for_a_tiny_topology() {
        let mut out = Vec::new();
        encode(&index_model(&[], &[]).expect("fits"), &mut out).expect("finite");
        assert_eq!(out, [0u8; 16], "four zero counts, nothing else");
        let mut opt = Vec::new();
        put_opt(&mut opt, Some("ab"));
        put_opt(&mut opt, None);
        assert_eq!(opt, [1, 2, 0, 0, 0, b'a', b'b', 0]);
        let mut list = Vec::new();
        put_u32s(&mut list, &[7, 0x0102_0304]);
        assert_eq!(list, [2, 0, 0, 0, 7, 0, 0, 0, 4, 3, 2, 1]);
    }

    #[test]
    fn the_seed_sizes_the_graph_at_two_plus_seed_mod_600_nodes() {
        for (seed, nodes) in [(0, 2), (5, 7), (599, 601), (600, 2), (1205, 7)] {
            let bytes = topology_stage(seed, REFERENCE_DEGREE).expect("fits");
            assert_eq!(bytes[..4], u32::to_le_bytes(nodes), "seed {seed}");
        }
    }

    #[test]
    fn the_adjacency_and_database_members_reach_the_bytes() {
        let nodes = [node("a", "db"), node("b", "db")];
        let encoded = |edges: &[EdgeRecord]| {
            let mut out = Vec::new();
            encode(&index_model(&nodes, edges).expect("fits"), &mut out).expect("finite");
            out
        };
        let (bare, linked) = (encoded(&[]), encoded(&[edge("e", "a", "b")]));
        // by_database: "db" then members [0, 1] closes both encodings.
        let tail = [2, 0, 0, 0, b'd', b'b', 2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0];
        assert!(bare.ends_with(&tail) && linked.ends_with(&tail));
        // Before it: b's in-row [0], then the two empty hierarchy rows.
        let rows = |out: &[u8]| out[..out.len() - tail.len()].to_vec();
        let last = [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
        assert!(rows(&linked).ends_with(&last));
        assert!(rows(&bare).ends_with(&[0; 16]));
    }
}
