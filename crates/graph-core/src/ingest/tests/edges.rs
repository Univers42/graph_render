//! Edges: cross-collection references, and the refusals the derivation makes.
//!
//! Split out of `build.rs` for the house's 300-line limit. A refusal is a document
//! fact, and every one of them is a case where the alternative is a graph that is
//! well-formed and wrong, with nothing in the output to say so.

use super::support::*;

#[test]
fn a_link_across_collections_names_the_referenced_records_own_collection() {
    let mut doc = one_of_each();
    doc.collections.push(Collection {
        id: "person".into(),
        name: "People".into(),
        title_field: "name".into(),
        fields: vec![field("name", "Name", Role::Title)],
    });
    let collection = &mut doc.collections[0];
    collection.fields[7]
        .link
        .as_mut()
        .expect("declared")
        .collection = "person".into();
    doc.records.push(Record {
        id: "p1".into(),
        collection: "person".into(),
        deleted: false,
        updated_at: 3,
        values: vec![("name".into(), JsonValue::Text("Ada".into()))],
    });
    *cell(&mut doc.records[0], "blocks") = JsonValue::List(vec![JsonValue::Text("p1".into())]);
    let graph = build(&doc).expect("derives");
    let relation = graph
        .edges
        .iter()
        .find(|e| e.kind == EdgeKind::Relation)
        .expect("the link role derives one");
    // The middle coordinate is the *referenced* record's collection. Reading it as the
    // referencing one would make every cross-collection link dangle.
    assert_eq!(relation.target, "rows:person:p1");
    let (_, topology) = build_topology(&doc).expect("indexes");
    // Four nodes now: r1, p1 and the two tag hubs. The hierarchy edge still dangles
    // (`r0` is still absent), so three of the four derived edges survive indexing.
    assert_eq!(topology.node_count(), 4);
    assert_eq!(topology.edge_count(), 3);
}
