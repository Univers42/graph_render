//! The eight roles and the two cardinalities, pinned by name and in order.

use super::support::*;

// ---------------------------------------------------------------- the roles

#[test]
fn there_are_exactly_eight_roles_named_as_the_phase_prompt_spells_them() {
    let names: Vec<&str> = Role::ALL.iter().map(|r| r.as_str()).collect();
    assert_eq!(
        names,
        [
            "title", "label", "group", "tags", "link", "scalar", "weight", "parent"
        ]
    );
    assert_eq!(Role::ALL.len(), 8);
}

#[test]
fn role_names_round_trip_and_an_unknown_name_is_refused() {
    for role in Role::ALL {
        assert_eq!(Role::from_name(role.as_str()), Some(role), "{role:?}");
    }
    assert_eq!(Role::from_name("Title"), None);
    assert_eq!(Role::from_name("multi_select"), None);
    assert_eq!(Role::from_name(""), None);
}

#[test]
fn every_role_name_is_a_distinct_lowercase_token() {
    let mut seen: Vec<&str> = Vec::new();
    for role in Role::ALL {
        let name = role.as_str();
        assert!(
            name.chars().all(|c| c.is_ascii_lowercase()),
            "{name} is not a lowercase token"
        );
        assert!(!seen.contains(&name), "{name} is declared twice");
        seen.push(name);
    }
}

#[test]
fn the_same_record_id_in_two_collections_is_two_records() {
    // `Record.id` is "stable id *within its collection*" (`ingest.rs`) and the derived node
    // id is `source:collection:record`, so `r1` in `task` and `r1` in `task2` are two
    // records with two distinct nodes. Keying uniqueness on the bare id refused a
    // document the contract permits, naming a record that was not the one at fault.
    let two = MINIMAL
        .replacen(
            r#""collections": ["#,
            r#""collections": [{"id": "task2", "name": "Tasks 2", "titleField": "name",
                 "fields": [{ "id": "name", "name": "Name", "role": "title", "link": null }]}, "#,
            1,
        )
        .replacen(
            r#""values": { "name": "Write", "state": "doing", "labels": ["wip"], "effort": 2 } }"#,
            r#""values": { "name": "Write", "state": "doing", "labels": ["wip"], "effort": 2 } },
    { "id": "r1", "collection": "task2", "deleted": false, "updatedAt": 1700000001,
      "values": { "name": "Write again" } }"#,
            1,
        );
    let doc = read(&two).expect("one id in two collections is two records");
    assert_eq!(
        doc.records
            .iter()
            .map(|r| (r.collection.as_str(), r.id.as_str()))
            .collect::<Vec<_>>(),
        [("task", "r1"), ("task2", "r1")]
    );
    // The negative control: the *same* id twice in one collection is still refused, with
    // the message byte-identical to the one `reader.rs` pins.
    assert_eq!(
        err(&MINIMAL.replace(
            r#""records": [
    { "id": "r1""#,
            r#""records": [
    { "id": "r1", "collection": "task", "deleted": false, "updatedAt": 1, "values": {} },
    { "id": "r1""#
        )),
        "records[1]: duplicate record id `r1`"
    );
}

#[test]
fn an_empty_coordinate_is_refused_naming_which_one() {
    // H5's check tested only `:`, so an empty `source`, collection id, record id or field
    // id read to node ids like `:task:r1` — the same "well-formed and wrong" the colon is
    // refused for, and the direction that matters: `parse_node_id` splits on the first two
    // colons and hands back `Some` with an empty segment, never `None`.
    for (document, coordinate) in [
        (
            MINIMAL.replace(r#""source": "rows""#, r#""source": """#),
            "source",
        ),
        (
            MINIMAL.replacen(r#""id": "task","#, r#""id": "", "#, 1),
            "collection id",
        ),
        (
            MINIMAL.replacen(r#""id": "r1""#, r#""id": """#, 1),
            "record id",
        ),
        (
            MINIMAL.replacen(r#""id": "name""#, r#""id": """#, 1),
            "field id",
        ),
    ] {
        assert_eq!(
            err(&document),
            format!("{coordinate}: an id coordinate cannot be empty"),
            "{coordinate}"
        );
    }
}

#[test]
fn a_record_id_made_of_separators_still_reads() {
    // The opposite half: a record id is the *last* segment, so a `:` in it round-trips
    // exactly (`parse_node_id`'s `splitn(3, ':')` rejoins the remainder). Empty is not
    // the same fault as punctuated, and neither is a reason to widen the grammar.
    for id in [":", "a:b", "::"] {
        let text = MINIMAL.replacen(
            r#"{ "id": "r1", "collection""#,
            &format!(r#"{{ "id": "{id}", "collection""#),
            1,
        );
        let doc = read(&text).unwrap_or_else(|e| panic!("record id {id:?} reads: {e}"));
        assert_eq!(doc.records[0].id, id);
    }
}

#[test]
fn a_link_or_parent_cell_naming_an_unknown_record_is_refused() {
    // A `link` field's declared collection and a `parent` field's own collection are the
    // universe a cell's ids must name. Nothing checked the *values* before: `ghost` named
    // a node id `rows:task:ghost` that no record defines, so the graph carried an edge to
    // a node no source ever described, and the output said nothing about it.
    let with_blocks = |ids: &str| {
        MINIMAL.replace(
            r#""values": { "name": "Write""#,
            &format!(r#""values": {{ "blocks": {ids}, "name": "Write""#),
        )
    };
    assert_eq!(
        err(&with_blocks(r#"["ghost"]"#)),
        "records[0].values.blocks: record `r1` names `ghost`, which is not a record of \
         collection `task`"
    );
    // A bare string is a reference too: a source whose links arrive singly writes one.
    assert_eq!(
        err(&with_blocks(r#""ghost""#)),
        "records[0].values.blocks: record `r1` names `ghost`, which is not a record of \
         collection `task`"
    );
    // A `parent` field declares no link, so its own collection is the universe.
    assert_eq!(
        err(&MINIMAL.replace(
            r#""values": { "name": "Write""#,
            r#""values": { "up": ["ghost"], "name": "Write""#
        )),
        "records[0].values.up: record `r1` names `ghost`, which is not a record of \
         collection `task`"
    );
    // A record that is merely absent from the cell is not this fault: absent, `null` and
    // `[]` all name nothing, which is legal data.
    for empty in [r#"[]"#, "null"] {
        assert!(read(&with_blocks(empty)).is_ok(), "{empty}");
    }
    assert!(read(MINIMAL).is_ok(), "no cell at all");
}

#[test]
fn a_deleted_record_is_still_a_legal_link_target() {
    // A reference to a deleted record is legitimate data: the derivation is what skips a
    // deleted target (review finding F-106), so the reader must not refuse the document
    // that says so. The record-id universe for the check therefore includes deleted
    // records — refusing here would make the reader stricter than the derivation.
    let text = MINIMAL
        .replace(
            r#""values": { "name": "Write", "state": "doing", "labels": ["wip"], "effort": 2 } }"#,
            r#""values": { "name": "Write", "state": "doing", "labels": ["wip"], "effort": 2 } },
    { "id": "r2", "collection": "task", "deleted": true, "updatedAt": 1, "values": {} }"#,
        )
        .replace(
            r#""values": { "name": "Write""#,
            r#""values": { "blocks": ["r2"], "name": "Write""#,
        );
    let doc = read(&text).expect("a reference to a deleted record reads");
    assert!(doc.records[1].deleted, "r2 is the deleted one");
}

#[test]
fn a_link_field_spellings_round_trip_and_unknown_is_none() {
    assert_eq!(Cardinality::from_name("one"), Some(Cardinality::One));
    assert_eq!(Cardinality::from_name("many"), Some(Cardinality::Many));
    assert_eq!(Cardinality::from_name("ONE"), None);
    assert_eq!(Cardinality::from_name(""), None);
    assert_eq!(Cardinality::Many.as_str(), "many");
    assert_eq!(Cardinality::One.as_str(), "one");
}
