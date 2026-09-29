use super::*;

#[test]
fn a_link_to_a_record_that_is_absent_or_deleted_is_stated_and_then_dropped_by_indexing() {
    // `r2` and `r0` are named but not present, so both edges dangle. The derivation
    // says so rather than inventing endpoints; indexing is where a dangling edge goes,
    // and the two counts are what pin that split.
    let graph = derived();
    assert_eq!(graph.edges.len(), 4, "the derivation states all four");
    let (_, topology) = build_topology(&one_of_each()).expect("indexes");
    assert_eq!(topology.node_count(), 3);
    assert_eq!(
        topology.edge_count(),
        2,
        "only the two tag edges have both endpoints"
    );
}

#[test]
fn a_symmetric_link_is_undirected_and_a_directed_one_is_not() {
    let mut doc = one_of_each();
    let link = &mut doc.collections[0].fields[7]
        .link
        .as_mut()
        .expect("declared");
    assert!(!link.symmetric, "the fixture starts directed");
    link.symmetric = true;
    let graph = build(&doc).expect("derives");
    let relation = graph
        .edges
        .iter()
        .find(|e| e.kind == EdgeKind::Relation)
        .expect("the link role derives one");
    assert!(!relation.directed);
    // An undirected edge's id orders its endpoints, so a symmetric A→B and a B→A are
    // one id — which is what makes "symmetric" mean anything at the id level.
    assert_eq!(relation.id, "rows:task:r1--rows:task:r2:relation:Blocks");
}

#[test]
fn a_directed_link_keeps_its_orientation_in_its_id() {
    let graph = derived();
    let relation = graph
        .edges
        .iter()
        .find(|e| e.kind == EdgeKind::Relation)
        .expect("the link role derives one");
    assert!(relation.directed, "symmetric: false means directed");
    assert_eq!(relation.id, "rows:task:r1->rows:task:r2:relation:Blocks");
    assert_eq!(
        (relation.source.as_str(), relation.target.as_str()),
        ("rows:task:r1", "rows:task:r2")
    );
    // And the label is the field's human name, not its id: the contract says `name` is
    // for diagnostics and nothing derives from it *except* the edge a link field draws,
    // which is the one place a human name is what a consumer wants to read.
    assert_eq!(relation.label, "Blocks");
}

#[test]
fn a_one_cardinality_link_reads_a_single_reference_and_a_many_does_not() {
    let mut doc = one_of_each();
    doc.collections[0].fields[7]
        .link
        .as_mut()
        .expect("declared")
        .cardinality = Cardinality::One;
    *cell(&mut doc.records[0], "blocks") = JsonValue::Text("r2".into());
    assert_eq!(roles::references(&doc, &doc.records[0], "blocks"), ["r2"]);
    // The same single value under `many` is *not* a one-element list: the declared
    // cardinality is the shape, and reading across it would hide a schema mistake.
    doc.collections[0].fields[7]
        .link
        .as_mut()
        .expect("declared")
        .cardinality = Cardinality::Many;
    assert!(roles::references(&doc, &doc.records[0], "blocks").is_empty());
    let graph = build(&doc).expect("derives");
    assert!(!graph.edges.iter().any(|e| e.kind == EdgeKind::Relation));
}

#[test]
fn a_parent_reads_a_bare_string_or_a_list_of_exactly_one() {
    let doc = one_of_each();
    assert_eq!(roles::parent(&doc, &doc.records[0]), Some("r0"));
    // A source whose references are always a collection writes a one-element list even
    // where the relationship is one-to-one, and the contract has to be able to say so.
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "up") = JsonValue::List(vec![JsonValue::Text("r0".into())]);
    assert_eq!(roles::parent(&doc, &doc.records[0]), Some("r0"));
    let graph = build(&doc).expect("derives");
    assert_eq!(
        graph
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Hierarchy)
            .count(),
        1
    );
}

#[test]
fn a_parent_that_is_not_exactly_one_reference_derives_no_hierarchy_edge() {
    // Two parents is two claims about a tree. Picking the first would drop a subtree
    // with nothing to say so, so nothing is derived instead.
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "up") = JsonValue::List(vec![
        JsonValue::Text("r0".into()),
        JsonValue::Text("r5".into()),
    ]);
    assert_eq!(roles::parent(&doc, &doc.records[0]), None);
    let graph = build(&doc).expect("derives");
    assert!(
        !graph.edges.iter().any(|e| e.kind == EdgeKind::Hierarchy),
        "{:?}",
        graph.edges
    );
    // A number is not a record id either, and is not read as one.
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "up") = JsonValue::Number(7.0);
    assert_eq!(roles::parent(&doc, &doc.records[0]), None);
}
