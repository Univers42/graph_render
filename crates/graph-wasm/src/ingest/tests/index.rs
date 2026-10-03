//! `read_records` then `index`, the open path, refuses exactly the documents `read` refuses,
//! with the same refusal, and keeps every record of the ones it accepts.

use super::{doc, edge_json, node_json};
use crate::ingest::{IngestError, index, read, read_records};

fn via_index(text: &str) -> Result<(u32, u32), IngestError> {
    let (nodes, edges) = read_records(text.as_bytes())?;
    index(&nodes, &edges).map(|t| (t.node_count(), t.edge_count()))
}

fn via_read(text: &str) -> Result<(u32, u32), IngestError> {
    read(text.as_bytes()).map(|(nodes, edges)| (nodes.len() as u32, edges.len() as u32))
}

#[test]
fn index_refuses_what_read_refuses_and_keeps_what_it_keeps() {
    let (a, b) = (node_json("a"), node_json("b"));
    let cases = [
        doc(&[a.clone(), a.clone()], &[]),
        doc(
            &[a.clone(), b.clone()],
            &[edge_json("e", "a", "b"), edge_json("e", "b", "a")],
        ),
        doc(std::slice::from_ref(&a), &[edge_json("e", "a", "ghost")]),
        doc(std::slice::from_ref(&a), &[edge_json("e", "ghost", "a")]),
        // index_model keeps the second `e`, the first being dropped: still one short.
        doc(
            &[a.clone(), b.clone()],
            &[edge_json("e", "a", "ghost"), edge_json("e", "a", "b")],
        ),
        doc(
            &[a.clone(), b.clone()],
            &[edge_json("e", "a", "b"), edge_json("f", "b", "b")],
        ),
        doc(&[], &[]),
    ];
    for text in &cases {
        assert_eq!(via_index(text), via_read(text), "{text}");
    }
    assert!(via_read(&cases[0]).is_err() && via_read(&cases[5]).is_ok());
}
