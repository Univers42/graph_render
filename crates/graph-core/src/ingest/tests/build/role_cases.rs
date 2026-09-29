use super::*;

#[test]
fn a_deleted_record_derives_nothing_at_all() {
    let mut doc = one_of_each();
    doc.records[0].deleted = true;
    let graph = build(&doc).expect("derives");
    assert!(graph.nodes.is_empty(), "{:?}", graph.nodes);
    assert!(graph.edges.is_empty(), "{:?}", graph.edges);
    let (_, topology) = build_topology(&doc).expect("an empty graph is a graph");
    assert_eq!(topology.node_count(), 0);
    assert_eq!(topology.edge_count(), 0);
}

#[test]
fn a_record_with_no_weight_gets_the_declared_default() {
    let mut doc = one_of_each();
    doc.records[0].values.retain(|(k, _)| k != "effort");
    assert_eq!(
        build(&doc).expect("derives").nodes[0].weight,
        DEFAULT_WEIGHT
    );
    assert_eq!(DEFAULT_WEIGHT, 0.5, "the default is a pinned convention");
}

#[test]
fn a_record_with_no_title_keeps_its_record_id_as_the_label() {
    let mut doc = one_of_each();
    doc.records[0].values.retain(|(k, _)| k != "name");
    assert_eq!(build(&doc).expect("derives").nodes[0].label, "r1");
}

#[test]
fn a_scalar_role_is_declared_and_read_by_nobody() {
    let doc = one_of_each();
    let scalar = doc
        .collection("task")
        .expect("declared")
        .field("body")
        .expect("declared");
    assert_eq!(scalar.role, Role::Scalar);
    // The cell is present and ignored: the derived graph is byte-for-byte the one a
    // document with no `body` field at all would produce.
    let mut without = one_of_each();
    without.collections[0].fields.retain(|f| f.id != "body");
    without.records[0].values.retain(|(k, _)| k != "body");
    assert_eq!(
        describe(&derived()),
        describe(&build(&without).expect("derives"))
    );
}

#[test]
fn a_record_with_no_values_still_derives_its_node() {
    let mut doc = one_of_each();
    doc.records[0].values.clear();
    let graph = build(&doc).expect("derives");
    assert_eq!(graph.nodes.len(), 1);
    assert_eq!(graph.nodes[0].label, "r1");
    assert_eq!(graph.nodes[0].weight, DEFAULT_WEIGHT);
    assert!(graph.edges.is_empty());
}

#[test]
fn an_empty_document_derives_an_empty_graph_that_indexes() {
    let doc = Ingest {
        version: graph_contract::ingest::VERSION,
        source: "rows".into(),
        collections: vec![],
        records: vec![],
    };
    let graph = build(&doc).expect("derives");
    assert!(graph.nodes.is_empty() && graph.edges.is_empty());
    assert_eq!(
        index_model(&graph.nodes, &graph.edges)
            .expect("indexes")
            .node_count(),
        0
    );
}

#[test]
fn the_derivation_is_a_pure_function_of_the_document() {
    assert_eq!(describe(&derived()), describe(&derived()));
    assert_eq!(derived(), derived());
}
