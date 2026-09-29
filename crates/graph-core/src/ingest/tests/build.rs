//! The one derivation: every node and edge a document produces, pinned line for
//! line, plus each refusal.

use super::support::*;

// ------------------------------------------------------------- the derivation

#[test]
fn one_record_derives_one_node_with_every_role_filled_in() {
    let graph = derived();
    assert_eq!(graph.nodes.len(), 3, "one record and two tag hubs");
    let n = &graph.nodes[0];
    assert_eq!(n.id, "rows:task:r1");
    assert_eq!(n.kind, NodeKind::Record);
    assert_eq!(n.database_id.as_deref(), Some("task"));
    assert_eq!(n.source, "rows");
    assert_eq!(n.label, "Write");
    assert_eq!(n.group.as_deref(), Some("n"));
    assert_eq!(n.weight, 2.0);
    assert_eq!(n.version, 1_700_000_000.0);
    assert!(!n.has_note);
    assert_eq!(n.icon, None);
}

#[test]
fn the_whole_derivation_is_pinned_line_for_line() {
    // The phase's proof in one string: every node and every edge, in derived order, with
    // every field the derivation sets. A changed role, default, kind or strength shows
    // up as a changed line.
    assert_eq!(
        describe(&derived()),
        concat!(
            "node rows:task:r1 Record label=\"Write\" group=Some(\"n\") weight=2 version=1700000000\n",
            "node tag:wip Tag label=\"wip\" group=None weight=0.5 version=0\n",
            "node tag:graph Tag label=\"graph\" group=None weight=0.5 version=0\n",
            "edge rows:task:r0--rows:task:r1:hierarchy: rows:task:r0 -> rows:task:r1 Hierarchy label=\"\" strength=2 directed=false\n",
            "edge rows:task:r1->rows:task:r2:relation:Blocks rows:task:r1 -> rows:task:r2 Relation label=\"Blocks\" strength=1 directed=true\n",
            "edge rows:task:r1--tag:wip:tag:wip rows:task:r1 -> tag:wip Tag label=\"wip\" strength=0.75 directed=false\n",
            "edge rows:task:r1--tag:graph:tag:graph rows:task:r1 -> tag:graph Tag label=\"graph\" strength=0.75 directed=false\n",
        )
    );
}

#[test]
fn every_derived_edge_id_is_the_grammars_own() {
    let graph = derived();
    let ids: Vec<&str> = graph.edges.iter().map(|e| e.id.as_str()).collect();
    // The relation edge's label is the field's *name*, and a directed edge keeps its
    // orientation in its id (`->`), so the two facts the contract declares — the
    // field's human name and the link's `symmetric` — are both visible in the id.
    assert_eq!(
        ids,
        [
            "rows:task:r0--rows:task:r1:hierarchy:",
            "rows:task:r1->rows:task:r2:relation:Blocks",
            "rows:task:r1--tag:wip:tag:wip",
            "rows:task:r1--tag:graph:tag:graph",
        ]
    );
}

#[test]
fn edges_come_out_in_a_fixed_order_hierarchy_then_relations_then_tags() {
    let kinds: Vec<EdgeKind> = derived().edges.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            EdgeKind::Hierarchy,
            EdgeKind::Relation,
            EdgeKind::Tag,
            EdgeKind::Tag
        ]
    );
}

#[test]
fn every_derived_strength_is_the_tables_own() {
    for edge in derived().edges {
        assert_eq!(edge.strength, edge_strength(edge.kind), "{}", edge.id);
    }
}

#[test]
fn a_tag_value_becomes_one_hub_shared_by_every_record_carrying_it() {
    let mut doc = one_of_each();
    doc.records.push(Record {
        id: "r3".into(),
        collection: "task".into(),
        deleted: false,
        updated_at: 2,
        values: vec![
            ("name".into(), JsonValue::Text("Second".into())),
            (
                "labels".into(),
                JsonValue::List(vec![JsonValue::Text("wip".into())]),
            ),
        ],
    });
    let graph = build(&doc).expect("derives");
    let hubs: Vec<&str> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Tag)
        .map(|n| n.id.as_str())
        .collect();
    // First-appearance order, not sorted: the document wrote `wip` before `graph`, and
    // a derivation that reorders would make a diff of two renderings a diff of two
    // sort orders rather than of two derivations.
    assert_eq!(hubs, ["tag:wip", "tag:graph"], "first-appearance order");
    // The hub is shared; the edges are not — one per record that carries the value.
    let wip: Vec<&str> = graph
        .edges
        .iter()
        .filter(|e| e.target == "tag:wip")
        .map(|e| e.source.as_str())
        .collect();
    assert_eq!(wip, ["rows:task:r1", "rows:task:r3"]);
}

#[test]
fn a_tag_hub_carries_its_value_as_its_label_and_nothing_else() {
    let graph = derived();
    let hub = graph
        .nodes
        .iter()
        .find(|n| n.id == "tag:wip")
        .expect("a hub");
    assert_eq!(hub.kind, NodeKind::Tag);
    assert_eq!(hub.label, "wip");
    assert_eq!(hub.source, "rows");
    assert_eq!(hub.database_id, None);
    assert_eq!(hub.group, None);
    assert_eq!(hub.weight, DEFAULT_WEIGHT);
    assert_eq!(hub.version, 0.0);
    assert!(!hub.has_note);
    assert_eq!(hub.icon, None);
}

#[test]
fn record_nodes_come_first_and_tag_hubs_after_them() {
    let kinds: Vec<NodeKind> = derived().nodes.iter().map(|n| n.kind).collect();
    assert_eq!(kinds, [NodeKind::Record, NodeKind::Tag, NodeKind::Tag]);
}

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
fn the_same_tag_value_twice_is_one_edge_and_one_hub() {
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "labels") = JsonValue::List(vec![
        JsonValue::Text("wip".into()),
        JsonValue::Text("wip".into()),
    ]);
    let graph = build(&doc).expect("derives");
    assert_eq!(
        graph.edges.iter().filter(|e| e.target == "tag:wip").count(),
        1
    );
    assert_eq!(
        graph
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::Tag)
            .count(),
        1
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
