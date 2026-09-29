//! The declared roles, read: what each one does and what it refuses.

use super::support::*;

// ------------------------------------------------------------------ the roles

#[test]
fn each_role_is_read_through_its_own_declared_field() {
    let doc = one_of_each();
    let record = &doc.records[0];
    assert_eq!(roles::label(&doc, record), Some("Write"));
    assert_eq!(roles::group(&doc, record), Some("n"), "the label role");
    // The `group` role is declared and read by the derivation's grouping, which is not a
    // column on NodeRecord: the label role is the group. Both are declared, both mean
    // something, and neither is read by name-matching.
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
    let mut doc = one_of_each();
    doc.records[0].values.retain(|(k, _)| k != "note");
    assert_eq!(roles::group(&doc, &doc.records[0]), None);
    let mut doc = one_of_each();
    *cell(&mut doc.records[0], "note") = JsonValue::Text(String::new());
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
