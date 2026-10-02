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
    assert_eq!(relation.id, "rows:task:r1--rows:task:r2:relation:blocks");
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
    assert_eq!(relation.id, "rows:task:r1->rows:task:r2:relation:blocks");
    assert_eq!(
        (relation.source.as_str(), relation.target.as_str()),
        ("rows:task:r1", "rows:task:r2")
    );
    // And the label is the field's **id**, not its human name. The contract says `name`
    // is "for diagnostics only. Nothing derives from it", so two adapters declaring the
    // same link under different display names (`blocks` / "Blocks", `refs` / "Refs")
    // derive one edge id and one label, which is what makes the convergence pair a proof
    // rather than a coincidence.
    assert_eq!(relation.label, "blocks");
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

#[test]
fn an_edge_to_a_deleted_record_is_never_dropped_by_the_derivation_itself() {
    // The `hierarchy` edge above and this one are the same rule read from both ends: the
    // derivation states the claim, indexing drops the edge whose endpoint no node
    // defines. Stated here so the *derivation's* half — that it neither invents nor
    // silently drops — is pinned on its own, and not only as the edge count that
    // `index_model` happens to produce.
    let mut doc = one_of_each();
    doc.records.push(Record {
        id: "r2".into(),
        collection: "task".into(),
        deleted: true,
        updated_at: 4,
        values: vec![("name".into(), JsonValue::Text("Gone".into()))],
    });
    let graph = build(&doc).expect("derives");
    let relation = graph
        .edges
        .iter()
        .find(|e| e.kind == EdgeKind::Relation)
        .expect("the link role derives one, deleted target or not");
    assert_eq!(relation.target, "rows:task:r2");
    assert!(
        !graph.nodes.iter().any(|n| n.id == "rows:task:r2"),
        "a deleted record derives no node, so that edge is the dangling kind"
    );
    let (_, topology) = build_topology(&doc).expect("indexes");
    assert_eq!(topology.edge_count(), 2, "only the two tag edges survive");
}

#[test]
fn a_parent_naming_a_deleted_record_is_stated_and_then_dropped_by_indexing() {
    // The derivation states what the document claims — a parent, live or not — and
    // `index_model` is where an edge whose endpoint no node defines goes. A hierarchy
    // edge naming a deleted record therefore derives exactly the graph the same document
    // derives with no `up` cell at all, and nothing is silently reparented.
    let mut doc = one_of_each();
    doc.records.push(Record {
        id: "r0".into(),
        collection: "task".into(),
        deleted: true,
        updated_at: 2,
        values: vec![("name".into(), JsonValue::Text("Gone".into()))],
    });
    let graph = build(&doc).expect("derives");
    let hierarchy: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Hierarchy)
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(hierarchy, ["rows:task:r0--rows:task:r1:hierarchy:"]);
    // One record node (the deleted one derives nothing) and two tag hubs; the hierarchy
    // and relation edges both dangle, so only the tag edges are indexed.
    let (_, topology) = build_topology(&doc).expect("indexes");
    assert_eq!(topology.node_count(), 3);
    assert_eq!(topology.edge_count(), 2);
}

#[test]
fn a_reference_carried_twice_in_one_many_link_is_one_edge() {
    // Same reason a tag value carried twice is one edge: the edge id would be identical
    // either way, and `index_model` keeps only the first, so the derivation states one
    // fact rather than asking a consumer to discover the duplicate.
    let mut doc = one_of_each();
    doc.records.push(Record {
        id: "r2".into(),
        collection: "task".into(),
        deleted: false,
        updated_at: 3,
        values: vec![("name".into(), JsonValue::Text("Next".into()))],
    });
    *cell(&mut doc.records[0], "blocks") = JsonValue::List(vec![
        JsonValue::Text("r2".into()),
        JsonValue::Text("r2".into()),
    ]);
    let graph = build(&doc).expect("derives");
    let relations: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Relation)
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(relations, ["rows:task:r1->rows:task:r2:relation:blocks"]);
}

#[test]
fn a_link_carried_by_both_records_is_one_edge() {
    // The same rule as the reference carried twice, one step wider: the two claims live in
    // two records rather than in one list. A symmetric `A→B` and a `B→A` are one id
    // (`make_edge_id` orders the endpoints of an undirected edge), so the derivation
    // states one edge rather than a pair that `index_model` would silently halve.
    let mut doc = one_of_each();
    doc.collections[0].fields[7]
        .link
        .as_mut()
        .expect("declared")
        .symmetric = true;
    doc.records.push(Record {
        id: "r2".into(),
        collection: "task".into(),
        deleted: false,
        updated_at: 3,
        values: vec![
            ("name".into(), JsonValue::Text("Next".into())),
            (
                "blocks".into(),
                JsonValue::List(vec![JsonValue::Text("r1".into())]),
            ),
        ],
    });
    let graph = build(&doc).expect("derives");
    let relations: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Relation)
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(relations, ["rows:task:r1--rows:task:r2:relation:blocks"]);
    // The indexed graph holds that same one relation edge, so the derivation and the
    // topology cannot disagree about how many facts there were: the four edges derived
    // are the hierarchy edge naming the absent `r0` (dangling, so indexing drops it),
    // the one relation edge, and the two tag edges.
    let (_, topology) = build_topology(&doc).expect("indexes");
    assert_eq!(
        topology.edge_count(),
        3,
        "one relation edge and two tag edges"
    );
}
