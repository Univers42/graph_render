//! The `GMX1` writer against the reader it exists for: encode these records, decode them
//! back, and every field must be the record it came from.
//!
//! Nothing here indexes the batch — that is `crate::ingest::columns` and graph-core's job —
//! so these tests are about the *bytes*: the section arithmetic, the string table, and the
//! two endpoint senses.

use super::*;
use graph_contract::ingest_columns::{Format, NodeRow, decode, decode_batch};
use graph_core::{EdgeKind, NodeKind};

/// Two nodes and the three edges a batch may carry: between its own rows, a loop, and one
/// endpoint that names a node the batch does **not** carry — the case a whole document
/// cannot express, and the reason this writer interns on demand.
fn batch() -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let node = |id: &str, weight: f64| NodeRecord {
        id: id.into(),
        kind: NodeKind::ALL[(id.len() + 1) % NodeKind::ALL.len()],
        database_id: (id == "n-1").then(|| "db-0".into()),
        source: "studio".into(),
        label: format!("label {id}"),
        group: (id == "n-0").then(|| "g-0".into()),
        weight,
        version: id.len() as f64,
        has_note: id == "n-1",
        icon: (id == "n-0").then(|| "i-0".into()),
    };
    let edge = |id: &str, source: &str, target: &str| EdgeRecord {
        id: id.into(),
        source: source.into(),
        target: target.into(),
        kind: EdgeKind::ALL[id.len() % EdgeKind::ALL.len()],
        label: format!("{id} label"),
        strength: 0.5,
        directed: true,
        record_id: (id == "e-1").then(|| "rec-7".into()),
        child_first: false,
    };
    (
        vec![node("n-0", -0.0), node("n-1", 1.0)],
        vec![
            edge("e-0", "n-0", "n-1"),
            edge("e-1", "n-0", "n-0"),
            edge("e-2", "n-0", "already-there"),
        ],
    )
}

#[test]
fn a_batch_round_trips_to_the_records_it_was_written_from() {
    let (nodes, edges) = batch();
    let bytes = columns_batch(&nodes, &edges);
    let doc = decode_batch(&bytes).expect("this writer writes a batch");
    assert_eq!(doc.format(), Format::Batch);
    assert_eq!((doc.node_count(), doc.edge_count()), (2, 3));
    for (row, node) in nodes.iter().enumerate() {
        assert_eq!(
            doc.node(row as u32),
            Some(NodeRow {
                id: &node.id,
                kind: node.kind.as_str(),
                database_id: node.database_id.as_deref(),
                source: &node.source,
                label: &node.label,
                group: node.group.as_deref(),
                weight: node.weight,
                version: node.version,
                has_note: node.has_note,
                icon: node.icon.as_deref(),
            }),
            "node row {row}"
        );
    }
    for (row, edge) in edges.iter().enumerate() {
        let read = doc.edge(row as u32).expect("edge row");
        assert_eq!(read.id, edge.id, "edge row {row}");
        assert_eq!(read.kind, edge.kind.as_str(), "edge row {row}");
        assert_eq!(read.label, edge.label, "edge row {row}");
        assert_eq!(read.record_id, edge.record_id.as_deref(), "edge row {row}");
        assert_eq!(read.directed, edge.directed, "edge row {row}");
        assert_eq!(read.child_first, edge.child_first, "edge row {row}");
        assert_eq!(read.strength.to_bits(), edge.strength.to_bits(), "edge row {row}");
        // The endpoints are entries, so what they mean is the text behind them.
        assert_eq!(doc.text(read.source_row), Some(edge.source.as_str()));
        assert_eq!(doc.text(read.target_row), Some(edge.target.as_str()));
    }
}

#[test]
fn a_batch_is_not_a_document_to_either_reader() {
    let (nodes, edges) = batch();
    let bytes = columns_batch(&nodes, &edges);
    assert!(
        decode(&bytes).is_err(),
        "the document reader refuses the batch's magic"
    );
    assert!(decode_batch(&document(&nodes, &edges).expect("finite").into_bytes()).is_err());
}

#[test]
fn an_endpoint_naming_a_node_the_batch_does_not_carry_is_still_a_string() {
    let (nodes, edges) = batch();
    let bytes = columns_batch(&nodes, &edges);
    let doc = decode_batch(&bytes).expect("an endpoint name is a name, not a row");
    let dangling = doc.edge(2).expect("the third edge");
    assert_eq!(doc.text(dangling.target_row), Some("already-there"));
    assert!(
        dangling.target_row >= doc.node_count(),
        "its entry is not a dense row, which is why a document could not carry it"
    );
}

#[test]
fn a_batch_of_a_seeded_model_round_trips_over_every_seed() {
    for seed in [0u32, 1, 5, 37] {
        let (nodes, edges) =
            seeded_model(seed, gate_node_count(seed), graph_core::REFERENCE_DEGREE);
        let bytes = columns_batch(&nodes, &edges);
        let doc = decode_batch(&bytes).expect("this writer writes a batch");
        assert_eq!(doc.node_count() as usize, nodes.len(), "seed {seed}");
        assert_eq!(doc.edge_count() as usize, edges.len(), "seed {seed}");
        for (row, node) in nodes.iter().enumerate() {
            assert_eq!(doc.node(row as u32).map(|n| n.id), Some(node.id.as_str()));
        }
        for (row, edge) in edges.iter().enumerate() {
            let read = doc.edge(row as u32).expect("edge row");
            assert_eq!(doc.text(read.source_row), Some(edge.source.as_str()));
            assert_eq!(doc.text(read.target_row), Some(edge.target.as_str()));
        }
    }
}

/// The writer allocates the buffer once, at the length the decoder's exact-size check
/// demands — so the two halves of the section table cannot drift without this failing.
#[test]
fn the_written_buffer_is_exactly_the_length_the_reader_demands() {
    let (nodes, edges) = batch();
    let bytes = columns_batch(&nodes, &edges);
    let short = &bytes[..bytes.len() - 1];
    assert!(decode_batch(short).is_err(), "a byte less is a different length");
    assert!(decode_batch(&[bytes.clone(), vec![0u8]].concat()).is_err());
}

/// An empty batch is a legal document of no rows, which is what an extend with nothing to
/// append sends.
#[test]
fn an_empty_batch_is_a_batch() {
    let bytes = columns_batch(&[], &[]);
    let doc = decode_batch(&bytes).expect("empty is valid");
    assert_eq!((doc.node_count(), doc.edge_count(), doc.string_count()), (0, 0, 0));
}