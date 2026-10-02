//! F-01: version 1 reads an omitted edge `child_first` as `false` (parent-first), which is
//! the documented contract; a present member must still be a boolean.

use super::{doc, edge_json, node_json};
use crate::ingest::{IngestError, read};

fn edge_with(child_first: &str) -> String {
    edge_json("e", "a", "b").replace('}', &format!(",\"child_first\":{child_first}}}"))
}

fn first_edge(edge: String) -> Result<bool, IngestError> {
    let text = doc(&[node_json("a"), node_json("b")], &[edge]);
    read(text.as_bytes()).map(|(_, edges)| edges[0].child_first)
}

#[test]
fn an_omitted_child_first_reads_parent_first_and_a_present_one_is_read() {
    assert_eq!(first_edge(edge_json("e", "a", "b")), Ok(false));
    assert_eq!(first_edge(edge_with("true")), Ok(true));
    assert_eq!(first_edge(edge_with("false")), Ok(false));
}

#[test]
fn a_child_first_that_is_not_a_boolean_is_refused() {
    for value in ["null", "1", "\"true\""] {
        let refused = first_edge(edge_with(value));
        assert!(
            matches!(refused, Err(IngestError::Shape(_))),
            "{value}: {refused:?}"
        );
    }
}

#[test]
fn the_abi_doc_states_what_an_omitted_child_first_means() {
    let doc = include_str!("../../../../../docs/contract/wasm-abi.md");
    let rule = "`child_first` is optional in version 1: omitted, it reads `false`";
    assert!(doc.contains(rule), "wasm-abi.md must state: {rule}");
}
