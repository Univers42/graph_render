//! The topology stage of the 4-way hash gate: a synthetic model, remixed by seed so every
//! part of the topology is populated, indexed, and written out as bytes — so native and
//! wasm32 are compared on everything the topology holds, not only on what the oracle's
//! benchmark graph happens to exercise.

use crate::arena::CapacityError;
use crate::edgekind::EdgeKind;
use crate::index::{Topology, index_model};
use crate::records::{EdgeRecord, NodeRecord};
use crate::synthetic::{Mulberry32, synthetic_records};
use crate::weights::apply_degree_weights_against;
use core::fmt;

/// Distinct sources the remix draws from: more than 256, so the group column crosses
/// the oracle's `Uint8Array` limit (H9) on every large enough seed.
const SOURCES: usize = 300;

/// Why a stage produced no bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageError {
    /// The topology did not fit the `u32` index space.
    Capacity(CapacityError),
    /// A float column held NaN or ±∞, whose bits wasm32 does not pin (D9).
    NonFinite {
        /// Which column.
        column: &'static str,
    },
}

impl fmt::Display for StageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Capacity(err) => err.fmt(f),
            Self::NonFinite { column } => write!(f, "non-finite value in column {column}"),
        }
    }
}

/// Bytes of the topology for `seed`: `2 + seed % 600` synthetic nodes, sources and edge
/// kinds redrawn from the seed, weights against `reference_degree`. Every column, every
/// CSR and every string is written in a fixed little-endian layout, so a divergence
/// anywhere in the topology changes the bytes.
pub fn topology_stage(seed: u32, reference_degree: u32) -> Result<Vec<u8>, StageError> {
    let (mut nodes, mut edges) = synthetic_records(2 + seed % 600);
    remix(seed, &mut nodes, &mut edges);
    apply_degree_weights_against(&mut nodes, &edges, reference_degree);
    let topology = index_model(&nodes, &edges).map_err(StageError::Capacity)?;
    let mut out = Vec::new();
    encode(&topology, &mut out)?;
    Ok(out)
}

fn remix(seed: u32, nodes: &mut [NodeRecord], edges: &mut [EdgeRecord]) {
    let mut rnd = Mulberry32::new(seed);
    for node in nodes {
        node.source = format!("s{}", rnd.pick(SOURCES));
    }
    for edge in edges {
        edge.kind = EdgeKind::ALL[rnd.pick(EdgeKind::ALL.len())];
    }
}

fn encode(t: &Topology, out: &mut Vec<u8>) -> Result<(), StageError> {
    let stats = t.stats();
    for count in [stats.nodes, stats.edges, stats.databases, stats.notes] {
        put_u32(out, count);
    }
    for i in 0..t.node_count() {
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
    }
    for e in 0..t.edge_count() {
        let (edge, columns) = (t.edge(e), t.edges());
        put_str(out, edge.id);
        put_u32(out, columns.source[e as usize]);
        put_u32(out, columns.target[e as usize]);
        out.extend([edge.kind as u8, u8::from(edge.directed)]);
        put_str(out, edge.label);
        put_opt(out, edge.record_id);
        put_f64(out, edge.strength, "strength")?;
    }
    for csr in [t.out(), t.inbound(), t.hierarchy()] {
        (0..csr.rows()).for_each(|r| put_u32s(out, csr.row(r)));
    }
    for (database, members) in t.by_database() {
        put_str(out, database);
        put_u32s(out, members);
    }
    Ok(())
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
    use crate::records::build::{edge, node};
    use crate::weights::REFERENCE_DEGREE;

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
    }
}
