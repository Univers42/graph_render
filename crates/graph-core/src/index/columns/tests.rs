use super::*;
use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;

/// A node row: `label` is derived from the id so a differential can see which row it read.
fn node<'a>(id: &'a str) -> NodeView<'a> {
    NodeView {
        id,
        kind: NodeKind::Record,
        database_id: Some("db"),
        source: "pg",
        label: "L",
        group: None,
        weight: 0.5,
        version: 0.0,
        has_note: false,
        icon: None,
    }
}

/// An edge row between two dense indices.
fn edge<'a>(id: &'a str, source_row: u32, target_row: u32) -> RowEdge<'a> {
    RowEdge {
        id,
        source_row,
        target_row,
        kind: EdgeKind::Relation,
        label: "",
        strength: 0.5,
        directed: false,
        record_id: None,
        child_first: false,
    }
}

fn indexed<'a>(nodes: &[NodeView<'a>], edges: &[RowEdge<'a>]) -> Result<Topology, ColumnsRefusal> {
    index_columns(nodes.iter().copied(), edges.iter().copied(), (0, 0))
}

/// The refusal a document earns. `Topology` is not `PartialEq` (it holds a whole arena and
/// three CSRs), so the error is pulled out and compared on its own.
fn refusal<'a>(nodes: &[NodeView<'a>], edges: &[RowEdge<'a>]) -> ColumnsRefusal {
    indexed(nodes, edges).expect_err("this document is refused")
}

#[test]
fn a_row_is_the_dense_index_and_the_endpoints_are_already_resolved() {
    let nodes = [node("a"), node("b"), node("c")];
    let edges = [edge("e0", 2, 0), edge("e1", 0, 2)];
    let t = indexed(&nodes, &edges).expect("every id is fresh");
    assert_eq!((t.node_count(), t.edge_count()), (3, 2));
    for row in 0..3u32 {
        assert_eq!(t.node(row).id, nodes[row as usize].id, "row {row}");
        assert_eq!(t.node_index(nodes[row as usize].id), Some(row));
    }
    assert_eq!((t.edge(0).source, t.edge(0).target), ("c", "a"));
    assert_eq!((t.out().row(2).len(), t.inbound().row(0).len()), (1, 1));
}

#[test]
fn the_same_id_under_two_string_indices_is_refused_not_merged() {
    // Two different `&str`s with the same bytes: what the arena compares, and what the
    // document's string table can produce by holding the value twice.
    let nodes = [node("a"), node("b"), node("a")];
    assert_eq!(
        refusal(&nodes, &[]),
        ColumnsRefusal::DuplicateNodeId { row: 2 }
    );
}

#[test]
fn a_duplicate_in_the_last_row_is_refused_too() {
    let nodes = [node("a"), node("b"), node("c"), node("a")];
    let edges = [edge("e0", 0, 1)];
    assert_eq!(
        refusal(&nodes, &edges),
        ColumnsRefusal::DuplicateNodeId { row: 3 }
    );
}

#[test]
fn an_edge_naming_the_row_of_the_second_duplicate_is_refused_not_attached_to_the_first() {
    // Row 2 repeats row 0's id. If the duplicate were dropped — first wins, as
    // `index_model` does — row 2 would vanish, the rows after it would shift down, and
    // this edge would silently point at whatever moved into row 2. Refusing the document is
    // the only answer that keeps "row r is index r" true.
    let nodes = [node("a"), node("b"), node("a"), node("c")];
    let edges = [edge("e0", 0, 1), edge("e1", 2, 3)];
    assert_eq!(
        refusal(&nodes, &edges),
        ColumnsRefusal::DuplicateNodeId { row: 2 }
    );
}

#[test]
fn a_repeated_edge_id_is_refused_and_claims_no_row() {
    let nodes = [node("a"), node("b")];
    let edges = [edge("e0", 0, 1), edge("e0", 1, 0)];
    assert_eq!(
        refusal(&nodes, &edges),
        ColumnsRefusal::DuplicateEdgeId { row: 1 }
    );
}

#[test]
fn an_endpoint_past_the_last_node_is_refused() {
    let nodes = [node("a"), node("b")];
    let at = ColumnsRefusal::EndpointRow { row: 0 };
    assert_eq!(refusal(&nodes, &[edge("e0", 0, 2)]), at);
    assert_eq!(refusal(&nodes, &[edge("e0", 9, 0)]), at);
}

#[test]
fn an_empty_document_indexes_to_the_empty_topology() {
    let t = indexed(&[], &[]).expect("nothing to refuse");
    assert_eq!((t.node_count(), t.edge_count()), (0, 0));
    assert_eq!(t.stats().nodes, 0);
}

#[test]
fn a_refusal_names_the_row_and_the_reason() {
    assert_eq!(
        ColumnsRefusal::DuplicateNodeId { row: 7 }.to_string(),
        "node row 7 repeats an id"
    );
    assert_eq!(
        ColumnsRefusal::DuplicateEdgeId { row: 2 }.to_string(),
        "edge row 2 repeats an id"
    );
    assert_eq!(
        ColumnsRefusal::EndpointRow { row: 1 }.to_string(),
        "edge row 1 names a node row that is gone"
    );
    assert_eq!(
        ColumnsRefusal::Capacity(CapacityError { what: "arena" }).to_string(),
        "arena exceeds the u32 index space"
    );
}
