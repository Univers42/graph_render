//! The one fixture every ingest test builds on: a document carrying all eight roles,
//! with a value for each.
//!
//! Every test states the document it means by building it here rather than by quoting
//! JSON, so a test reads as "the `label` role is the node's `group`" instead of as a
//! wall of text whose shape is itself untested.
//!
//! The re-exports below are what let each test module be a single `use
//! super::support::*;`: one place says which vocabulary every test shares, and a glob
//! import never warns about an unused member.

pub(super) use crate::ingest::{
    DEFAULT_WEIGHT, Derived, build, build_topology, describe, edge_strength, roles,
    to_canonical_json,
};
pub(super) use crate::{EdgeKind, NodeKind, index_model, parse_node_id};
pub(super) use graph_contract::ingest::{
    Cardinality, Collection, Field, Ingest, JsonValue, Link, Record, Role,
};

/// One collection carrying all eight roles, and one record with a value for each. Built
/// in Rust rather than read from text, so a test states the shape it means.
pub(super) fn one_of_each() -> Ingest {
    Ingest {
        version: graph_contract::ingest::VERSION,
        source: "rows".into(),
        collections: vec![Collection {
            id: "task".into(),
            name: "Tasks".into(),
            title_field: "name".into(),
            fields: vec![
                field("name", "Name", Role::Title),
                field("note", "Note", Role::Label),
                field("group", "Group", Role::Group),
                field("labels", "Labels", Role::Tags),
                field("effort", "Effort", Role::Weight),
                field("up", "Up", Role::Parent),
                field("body", "Body", Role::Scalar),
                link_field(),
            ],
        }],
        records: vec![Record {
            id: "r1".into(),
            collection: "task".into(),
            deleted: false,
            updated_at: 1_700_000_000,
            values: vec![
                ("name".into(), JsonValue::Text("Write".into())),
                ("note".into(), JsonValue::Text("n".into())),
                ("group".into(), JsonValue::Text("doing".into())),
                (
                    "labels".into(),
                    JsonValue::List(vec![
                        JsonValue::Text("wip".into()),
                        JsonValue::Text("graph".into()),
                    ]),
                ),
                ("effort".into(), JsonValue::Number(2.0)),
                ("up".into(), JsonValue::Text("r0".into())),
                ("body".into(), JsonValue::Text("ignored".into())),
                (
                    "blocks".into(),
                    JsonValue::List(vec![JsonValue::Text("r2".into())]),
                ),
            ],
        }],
    }
}

pub(super) fn field(id: &str, name: &str, role: Role) -> Field {
    Field {
        id: id.into(),
        name: name.into(),
        role,
        link: None,
    }
}

/// The one `link` field of the fixture, as a value: the three-argument `field` above
/// cannot express it, and a test that spells the whole struct eight times says less
/// than one that names the link it means.
pub(super) fn link_field() -> Field {
    Field {
        id: "blocks".into(),
        name: "Blocks".into(),
        role: Role::Link,
        link: Some(Link {
            collection: "task".into(),
            cardinality: Cardinality::Many,
            symmetric: false,
        }),
    }
}

pub(super) fn derived() -> Derived {
    build(&one_of_each()).expect("the document derives")
}

pub(super) fn cell<'a>(record: &'a mut Record, key: &str) -> &'a mut JsonValue {
    &mut record
        .values
        .iter_mut()
        .find(|(k, _)| k == key)
        .expect("declared")
        .1
}
