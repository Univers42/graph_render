//! One node or edge element, read out of the document's own text.
//!
//! The reader this replaces took the element as a `canonical_json::Value` and moved each
//! string out of it; this one has the element's members located in place, by the walk that
//! reads the array they sit in ([`table::Element`]). The two agree member for member: every
//! key is checked against the shape *before* any member is read (so a stray `hasNote` is a
//! loud refusal, not a dropped extra), `kind` is read before any other member, the rest are
//! read in field order whatever order the document wrote them in, and an edge's optional
//! `child_first` is read last.
//!
//! A record's members land in a fixed-size table on the stack — nothing allocated per
//! member — and are still *read* in the fixed order afterwards, which is what makes a node
//! with two bad members refuse for the same one a reader that built a `Value` refused for.
//! The table holds each string member's own text as well as its span, because the walk has
//! just read those bytes: lexing them again per field was among the largest costs the walk
//! could give away (`docs/measurements/perf-p4d-extend.md`).

use super::IngestError;
use super::at::At;
use graph_core::{EdgeRecord, NodeRecord};

mod table;
pub(in crate::ingest) use table::Element;

/// Field names a node record has, exactly. Module-level rather than a `let` inside the
/// function (house limit: the array was most of what pushed it past 40 lines).
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

/// Field offsets into [`NODE_FIELDS`] and [`EDGE_FIELDS`], named so a struct literal reads
/// as the field list does. The two shapes share an offset only where they share a name at
/// the same place — `id`, `label` and `child_first`'s neighbour `kind` do not line up, which
/// is why `kind` has one constant per shape: it is 1 in a node and 3 in an edge, and both
/// are read before any other member.
pub(super) mod field {
    pub(super) const ID: usize = 0;

    // A node: id, kind, database_id, source, label, group, weight, version, has_note, icon.
    pub(super) const NODE_KIND: usize = 1;
    pub(super) const DATABASE_ID: usize = 2;
    pub(super) const NODE_SOURCE: usize = 3;
    pub(super) const NODE_LABEL: usize = 4;
    pub(super) const GROUP: usize = 5;
    pub(super) const WEIGHT: usize = 6;
    pub(super) const VERSION: usize = 7;
    pub(super) const HAS_NOTE: usize = 8;
    pub(super) const ICON: usize = 9;

    // An edge: id, source, target, kind, label, strength, directed, record_id, child_first.
    pub(super) const EDGE_SOURCE: usize = 1;
    pub(super) const EDGE_TARGET: usize = 2;
    pub(super) const EDGE_KIND: usize = 3;
    pub(super) const EDGE_LABEL: usize = 4;
    pub(super) const STRENGTH: usize = 5;
    pub(super) const DIRECTED: usize = 6;
    pub(super) const RECORD_ID: usize = 7;
    pub(super) const CHILD_FIRST: usize = 8;
}

/// The record shape [`read_all`](super::read_all) reads one list with: the members it
/// names, and the record those members make.
pub(super) trait Shape: Sized {
    /// The members this shape names, in the order they are read.
    const FIELDS: &'static [&'static str];

    /// The record the members located in `element` make, or the refusal naming `at`.
    fn read(element: &mut Element<'_>, at: At) -> Result<Self, IngestError>;
}

impl Shape for NodeRecord {
    const FIELDS: &'static [&'static str] = &NODE_FIELDS;

    fn read(element: &mut Element<'_>, at: At) -> Result<Self, IngestError> {
        // `kind` first, exactly as the reader that built a `Value` read it: a node naming
        // no known kind is refused before any other member of it is looked at.
        let kind = element.node_kind(at)?;
        Ok(NodeRecord {
            id: element.string(field::ID, "id")?,
            kind,
            database_id: element.opt_string(field::DATABASE_ID, "database_id")?,
            source: element.string(field::NODE_SOURCE, "source")?,
            label: element.string(field::NODE_LABEL, "label")?,
            group: element.opt_string(field::GROUP, "group")?,
            weight: element.number(field::WEIGHT, "weight")?,
            version: element.number(field::VERSION, "version")?,
            has_note: element.boolean(field::HAS_NOTE, "has_note")?,
            icon: element.opt_string(field::ICON, "icon")?,
        })
    }
}

impl Shape for EdgeRecord {
    const FIELDS: &'static [&'static str] = &EDGE_FIELDS;

    fn read(element: &mut Element<'_>, at: At) -> Result<Self, IngestError> {
        let kind = element.edge_kind(at)?;
        Ok(EdgeRecord {
            id: element.string(field::ID, "id")?,
            source: element.string(field::EDGE_SOURCE, "source")?,
            target: element.string(field::EDGE_TARGET, "target")?,
            kind,
            label: element.string(field::EDGE_LABEL, "label")?,
            strength: element.number(field::STRENGTH, "strength")?,
            directed: element.boolean(field::DIRECTED, "directed")?,
            record_id: element.opt_string(field::RECORD_ID, "record_id")?,
            // Optional, and read last in field position: an edge written before p3's
            // hierarchy direction reads as parent-first, the same default as `graph-cli`'s
            // `oracle_fixtures/wire.rs`. An edge with both a bad `directed` and a bad
            // `child_first` is refused for `directed`.
            child_first: match element.written(field::CHILD_FIRST) {
                true => element.boolean(field::CHILD_FIRST, "child_first")?,
                false => false,
            },
        })
    }
}
