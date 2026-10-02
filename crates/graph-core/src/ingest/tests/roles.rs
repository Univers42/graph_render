//! The declared roles, read: what each one does and what it refuses.

use super::support::*;

// ------------------------------------------------------------------ the roles

#[test]
fn each_role_is_read_through_its_own_declared_field() {
    let doc = one_of_each();
    let record = &doc.records[0];
    assert_eq!(roles::label(&doc, record), Some("Write"));
    // The `label` role is the node's group: that is what the contract's `Role::Label`
    // says it is. The `group` role is the fallback for a collection that declares no
    // label role at all (see the test below), never the winner over a declared one.
    assert_eq!(roles::group(&doc, record), Some("n"), "the label role");
    assert_eq!(
        roles::link_fields(doc.collection("task").expect("declared"))
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["blocks"]
    );
    assert_eq!(roles::weight(&doc, record), 2.0);
    assert_eq!(roles::parent(&doc, record), Some("r0"));
    assert_eq!(roles::tags(&doc, record), ["wip", "graph"]);
    assert_eq!(roles::references(&doc, record, "blocks"), ["r2"]);
}

#[test]
fn a_group_role_field_is_read_when_the_collection_declares_no_label_role() {
    // `Role::Group` is a declared role, so a document may declare it and expect it read.
    // Before, it had no reader at all: a field declared `group` was silently dropped and
    // every node came out with `group: null`.
    let doc = group_role_only();
    let record = &doc.records[0];
    assert_eq!(roles::group(&doc, record), Some("doing"), "the group role");
    assert_eq!(
        build(&doc).expect("derives").nodes[0].group.as_deref(),
        Some("doing"),
        "and it reaches the node"
    );
}

/// `one_of_each` with no `label`-role field: what a collection whose only grouping
/// declaration is `role: "group"` looks like, which is the case the fallback exists for.
fn group_role_only() -> Ingest {
    let mut doc = one_of_each();
    doc.collections[0].fields.retain(|f| f.role != Role::Label);
    doc
}

/// The smallest document that can answer "which field with this role comes first": two
/// of them, declared in the order given, each value being its own field id. Nothing else
/// about it can reach the answer.
fn two_labels(declared: [&str; 2]) -> Ingest {
    Ingest {
        version: graph_contract::ingest::VERSION,
        source: "rows".into(),
        collections: vec![Collection {
            id: "task".into(),
            name: "Tasks".into(),
            title_field: "name".into(),
            fields: vec![
                field("name", "Name", Role::Title),
                field(declared[0], "Zero", Role::Label),
                field(declared[1], "One", Role::Label),
            ],
        }],
        records: vec![Record {
            id: "r1".into(),
            collection: "task".into(),
            deleted: false,
            updated_at: 1,
            values: vec![
                ("name".into(), JsonValue::Text("Write".into())),
                (declared[0].into(), JsonValue::Text(declared[0].into())),
                (declared[1].into(), JsonValue::Text(declared[1].into())),
            ],
        }],
    }
}

#[test]
fn the_first_field_with_a_role_is_the_lowest_id_whatever_order_they_were_declared_in() {
    // H6, from the other direction: the order a document happened to list its fields in
    // must not reach a derived graph. Every `Ingest` field is `pub`, so a document built
    // in Rust — an adapter, a test, the SDK — carries no reader-side sort to save it.
    let highest_first = two_labels(["zz", "aa"]);
    assert_eq!(
        roles::group(&highest_first, &highest_first.records[0]),
        Some("aa"),
        "the lowest id, not the first declared"
    );
    assert_eq!(
        build(&highest_first).expect("derives").nodes[0]
            .group
            .as_deref(),
        Some("aa"),
        "and the derivation agrees with the reader"
    );
    let lowest_first = two_labels(["aa", "zz"]);
    assert_eq!(
        roles::group(&lowest_first, &lowest_first.records[0]),
        Some("aa")
    );
    assert_eq!(
        describe(&build(&highest_first).expect("derives")),
        describe(&build(&lowest_first).expect("derives")),
        "two declaration orders, one derivation"
    );
}

#[test]
fn link_fields_come_out_in_canonical_id_order_not_declaration_order() {
    let mut doc = one_of_each();
    // `aardvark` is declared *after* `blocks` and sorts before it, so declaration order
    // and canonical order disagree.
    let mut second = link_field();
    second.id = "aardvark".into();
    doc.collections[0].fields.push(second);
    assert_eq!(
        roles::link_fields(doc.collection("task").expect("declared"))
            .map(|f| f.id.as_str())
            .collect::<Vec<_>>(),
        ["aardvark", "blocks"]
    );
}

#[test]
fn a_weight_is_derived_as_declared_and_is_never_clamped_to_the_zero_one_convention() {
    // `NodeRecord.weight` documents 0..1 *by convention*, and both committed source
    // fixtures declare 3, 5 and 8. Clamping would silently rewrite every weight in the
    // convergence dataset (three nodes to 1.0 each) and refusing would make the phase's
    // own fixture unbuildable, so the declared number is what derives.
    let doc = one_of_each();
    assert_eq!(
        roles::weight(&doc, &doc.records[0]),
        2.0,
        "the fixture's 2.0"
    );
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "effort") = JsonValue::Number(-1.0);
    assert_eq!(
        roles::weight(&doc, &doc.records[0]),
        -1.0,
        "declared, not clamped, not refused"
    );
}

#[test]
fn the_title_comes_from_the_collections_title_field_not_from_searching_for_a_role() {
    let mut doc = one_of_each();
    let collection = &mut doc.collections[0];
    collection.title_field = "note".into();
    collection.fields[0].role = Role::Scalar;
    collection.fields[1].role = Role::Title;
    let record = &doc.records[0];
    assert_eq!(
        roles::label(&doc, record),
        Some("n"),
        "titleField names the field"
    );
}

#[test]
fn an_absent_value_is_none_and_an_empty_one_is_some_empty() {
    // The `label` role's cell (`note`), which is the node's group.
    let mut doc = group_role_only();
    doc.records[0]
        .values
        .retain(|(k, _)| k != "note" && k != "group");
    assert_eq!(roles::group(&doc, &doc.records[0]), None);
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "note") = JsonValue::Text(String::new());
    assert_eq!(roles::group(&doc, &doc.records[0]), Some(""));
    // And the same two answers from the `group` role, for a collection that declares no
    // label role at all.
    let mut doc = group_role_only();
    doc.records[0].values.retain(|(k, _)| k != "group");
    assert_eq!(roles::group(&doc, &doc.records[0]), None);
    let mut doc = group_role_only();
    *cell(&mut doc.records[0], "group") = JsonValue::Text(String::new());
    assert_eq!(roles::group(&doc, &doc.records[0]), Some(""));
}

#[test]
fn a_value_of_the_wrong_json_type_is_absent_not_a_lossy_conversion() {
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "name") = JsonValue::Number(7.0);
    *cell(&mut doc.records[0], "note") = JsonValue::Bool(true);
    *cell(&mut doc.records[0], "effort") = JsonValue::Text("2".into());
    *cell(&mut doc.records[0], "labels") = JsonValue::Text("wip".into());
    let record = &doc.records[0];
    assert_eq!(roles::label(&doc, record), None);
    assert_eq!(roles::group(&doc, record), None);
    assert_eq!(roles::weight(&doc, record), DEFAULT_WEIGHT);
    assert!(roles::tags(&doc, record).is_empty());
}

#[test]
fn a_tag_value_that_is_not_a_string_is_skipped_rather_than_stringified() {
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "labels") = JsonValue::List(vec![
        JsonValue::Text("wip".into()),
        JsonValue::Number(7.0),
        JsonValue::Null,
        JsonValue::Text("graph".into()),
    ]);
    assert_eq!(roles::tags(&doc, &doc.records[0]), ["wip", "graph"]);
}

#[test]
fn a_record_naming_a_collection_that_does_not_exist_reads_as_nothing() {
    let doc = one_of_each();
    let orphan = Record {
        id: "r9".into(),
        collection: "nope".into(),
        ..doc.records[0].clone()
    };
    assert_eq!(roles::label(&doc, &orphan), None);
    assert_eq!(roles::weight(&doc, &orphan), DEFAULT_WEIGHT);
    assert!(roles::tags(&doc, &orphan).is_empty());
}
