//! One node or edge element, moved out of the parsed document by value. The reader that
//! used to borrow the tree and `.to_owned()` every field is `super`; these two take their
//! element and hand out its strings.

use super::At;
use super::{IngestError, Slots, boolean, number, opt_string, require_only, shape, string};
use graph_contract::canonical_json::Value;
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

/// Field names `node`/`edge` require, exactly (`require_only`). Module-level rather than
/// a `let` inside each function (house limit: the array itself was most of what pushed
/// both functions past 40 lines).
pub(super) const NODE_FIELDS: [&str; 10] = [
    "id",
    "kind",
    "database_id",
    "source",
    "label",
    "group",
    "weight",
    "version",
    "has_note",
    "icon",
];

pub(super) const EDGE_FIELDS: [&str; 9] = [
    "id",
    "source",
    "target",
    "kind",
    "label",
    "strength",
    "directed",
    "record_id",
    "child_first",
];

pub(super) fn node(value: Value, at: At) -> Result<NodeRecord, IngestError> {
    let mut members = Slots::new(value, at)?;
    require_only(members.keys(), &NODE_FIELDS, at)?;
    // `kind` first, exactly as the borrowing reader read it: a node naming no known kind
    // is refused before any other member of it is looked at.
    let kind_name = string(members.take("kind", at)?, at.field("kind"))?;
    let kind = NodeKind::from_name(&kind_name).ok_or_else(|| {
        shape(
            at.field("kind"),
            &format!("unknown node kind {kind_name:?}"),
        )
    })?;
    Ok(NodeRecord {
        id: string(members.take("id", at)?, at.field("id"))?,
        kind,
        database_id: opt_string(members.take("database_id", at)?, at.field("database_id"))?,
        source: string(members.take("source", at)?, at.field("source"))?,
        label: string(members.take("label", at)?, at.field("label"))?,
        group: opt_string(members.take("group", at)?, at.field("group"))?,
        weight: number(members.take("weight", at)?, at.field("weight"))?,
        version: number(members.take("version", at)?, at.field("version"))?,
        has_note: boolean(members.take("has_note", at)?, at.field("has_note"))?,
        icon: opt_string(members.take("icon", at)?, at.field("icon"))?,
    })
}

pub(super) fn edge(value: Value, at: At) -> Result<EdgeRecord, IngestError> {
    let mut members = Slots::new(value, at)?;
    require_only(members.keys(), &EDGE_FIELDS, at)?;
    let kind_name = string(members.take("kind", at)?, at.field("kind"))?;
    let kind = EdgeKind::from_name(&kind_name).ok_or_else(|| {
        shape(
            at.field("kind"),
            &format!("unknown edge kind {kind_name:?}"),
        )
    })?;
    // Optional: an edge document written before p3's hierarchy direction reads as
    // parent-first, the same default as `graph-cli`'s `oracle_fixtures/wire.rs`. Read in
    // its field position, last, exactly as the borrowing reader read it: an edge with both
    // a bad `directed` and a bad `child_first` is refused for `directed`.
    //
    // A struct literal, no `..` (C13): p3's `child_first` field must break this build.
    Ok(EdgeRecord {
        id: string(members.take("id", at)?, at.field("id"))?,
        source: string(members.take("source", at)?, at.field("source"))?,
        target: string(members.take("target", at)?, at.field("target"))?,
        kind,
        label: string(members.take("label", at)?, at.field("label"))?,
        strength: number(members.take("strength", at)?, at.field("strength"))?,
        directed: boolean(members.take("directed", at)?, at.field("directed"))?,
        record_id: opt_string(members.take("record_id", at)?, at.field("record_id"))?,
        child_first: match members.take_optional("child_first") {
            Some(value) => boolean(value, at.field("child_first"))?,
            None => false,
        },
    })
}
