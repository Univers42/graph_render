use super::*;

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
