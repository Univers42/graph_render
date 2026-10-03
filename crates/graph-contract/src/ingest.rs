//! The ingest contract: what a data source hands the motor, and nothing else.
//!
//! Two faces of the same idea, exactly as `prompt.md` §4.1 requires. The **types**
//! below are the semantic face — the vocabulary every adapter writes and
//! `docs/contract/ingest-schema.json` describes. The **strict reader** ([`read`]) and
//! the **canonical writer** ([`to_json`]) are the two directions of the wire text, and
//! they are the *only* ones: an adapter is a mapping into [`Ingest`], never a place
//! where a graph is built.
//!
//! ## Why declared roles, not sniffed types
//!
//! The code this replaces reverse-engineered roles from a host's property type strings
//! — "the first `multi_select`, or a field named `/^tags?$/i`"; "the first `status`,
//! else the first `select`". A 20-member vendor type enum coupled the engine to one
//! vendor, and a heuristic that picks a role *silently changes the graph*. So a source
//! **declares** what each field means, in one of [`Role::ALL`] — eight roles, replacing
//! twenty vendor types — and the motor reads only the declaration. Declaring is also
//! what fixes **H6**: a value once found by position in `Object.values()` no longer
//! depends on member order, which is not stable across a JSON reserialization.
//!
//! ## Eight roles, each with one job
//!
//! | Role | What the motor reads it for |
//! |---|---|
//! | [`Title`](Role::Title) | the node's label; the collection's `titleField` names it |
//! | [`Label`](Role::Label) | a second string, the node's `group` |
//! | [`Group`](Role::Group) | a facet string; a grouping key with no hub of its own |
//! | [`Tags`](Role::Tags) | a string list; one tag hub node and one edge per value |
//! | [`Link`](Role::Link) | record references; one `relation` edge per reference |
//! | [`Scalar`](Role::Scalar) | anything else; carried, never structural |
//! | [`Weight`](Role::Weight) | a number; the node's visual weight |
//! | [`Parent`](Role::Parent) | a record reference; a `hierarchy` edge |
//!
//! [`Title`](Role::Title) and [`Label`](Role::Label) are separate on purpose: one source
//! has a `title` and a `rich_text`, another a `name` and a `slug`. Both are strings.
//!
//! ## What the reader refuses, and why each refusal is loud
//!
//! Every member is named and required, an unknown member refuses the whole document (a
//! stray camelCase is a *mistake*, not an extension), a role outside the eight is
//! refused rather than defaulted, and a duplicate collection, field or record id is
//! refused rather than first-wins. Each is a case where the alternative is a graph that
//! is well-formed and wrong, with nothing in the output to say so.
//!
//! Two things a document may do that are **not** refusals, because both are legal data a
//! real source produces:
//!
//! - **Two fields of one collection share a role.** [`Role::Scalar`] exists so a source
//!   can declare every field it has, so this is normal input. The canonically lowest field
//!   id wins, because the reader sorts a collection's fields by id before the derivation
//!   reads them; a later same-role field's value is carried and never read.
//! - **A link or parent cell names the record's own id.** The document derives a self
//!   edge, and a self hierarchy edge, from it. The checks below police *dangling*
//!   references, not cycles; nothing here walks a hierarchy looking for a loop.
//!
//! # The id grammar (H5) — decided: constrain, do not re-grammar
//!
//! `make_record_node_id` joins `source`, the collection id and the record id with `:`
//! and `parse_node_id` splits on the first two colons, so `:` inside `source` or a
//! collection id **cannot round-trip** — the parse returns a shifted, wrong answer
//! rather than `None` (H5, `ids.rs:61-66`). Broadening the grammar would move every
//! existing node id, and a node id that moves is a layout that moves, so that is a
//! stop-and-ask this phase does not take. This one constrains instead: `validate`
//! refuses a `source` or collection id containing `:`, naming the coordinate. A record
//! id may contain `:` freely — it is the last segment, so it round-trips.
//!
//! The Ponytail rides forward unchanged: `source`/`databaseId` containing `:` still
//! cannot round-trip *if a caller bypasses this validator*, and the parse still returns
//! a shifted wrong result rather than `null`. The escape hatch is `validate` itself —
//! a caller that builds ids another way has opted out of the guarantee knowingly.

mod collection;
mod error;
mod read;
#[cfg(feature = "codegen")]
pub mod schema;
#[cfg(test)]
mod tests;
mod validate;
mod write;

pub use collection::Collection;
pub use error::IngestError;

pub use read::read;
pub use write::{read_value, to_json, to_json_value};

/// The only ingest version this reader accepts.
pub const VERSION: u32 = 1;

/// What a declared field means. Eight roles, and a ninth is a stop-and-ask: every role
/// is a permanent public-surface commitment (the phase prompt, "Stop-and-ask").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// The record's own name; the collection's `titleField` names the field with this
    /// role, and its value is the node's label.
    Title,
    /// A second string, the node's `group`.
    Label,
    /// A facet string; a grouping key with no hub nodes of its own.
    Group,
    /// A list of strings; one tag hub node and one `tag` edge per value.
    Tags,
    /// References to records in another (or the same) collection; one `relation` edge
    /// per referenced record. A `link` field **must** declare its [`Link`].
    Link,
    /// Anything that is not structural: carried so a source can declare all of its
    /// fields, never read by the derivation.
    Scalar,
    /// A number; the node's visual weight.
    Weight,
    /// A reference to the record's parent; a `hierarchy` edge.
    Parent,
}

impl Role {
    /// Every role, in declaration order — the order the schema lists and the test pins.
    pub const ALL: [Self; 8] = [
        Self::Title,
        Self::Label,
        Self::Group,
        Self::Tags,
        Self::Link,
        Self::Scalar,
        Self::Weight,
        Self::Parent,
    ];

    /// The wire name, lowercase, as `docs/contract/ingest-schema.json` spells it.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Label => "label",
            Self::Group => "group",
            Self::Tags => "tags",
            Self::Link => "link",
            Self::Scalar => "scalar",
            Self::Weight => "weight",
            Self::Parent => "parent",
        }
    }

    /// The role named `name`, or `None` — never a default, so a typo is a refusal.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == name)
    }
}

/// How many records a [`Link`] may name: exactly one, or any number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Cardinality {
    /// A single record reference.
    One,
    /// A list of record references.
    Many,
}
impl Cardinality {
    /// Both cardinalities, in schema order.
    pub const ALL: [Self; 2] = [Self::One, Self::Many];

    /// The wire name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::One => "one",
            Self::Many => "many",
        }
    }

    /// The cardinality named `name`, or `None`.
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|card| card.as_str() == name)
    }
}

/// What a [`Role::Link`] field points at.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    /// Id of the collection the references name.
    pub collection: String,
    /// How many references the field may hold.
    pub cardinality: Cardinality,
    /// Whether the relation is undirected: an `A → B` and a `B → A` are one edge, and
    /// the edge is not drawn with an arrowhead.
    pub symmetric: bool,
}

/// One declared field of a collection.
#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    /// Stable id, unique within its collection; the key a record's `values` uses.
    pub id: String,
    /// Human name, for diagnostics only. Nothing derives from it.
    pub name: String,
    /// What the field means. Declared, never inferred.
    pub role: Role,
    /// The link's target, when `role` is [`Role::Link`] and only then.
    pub link: Option<Link>,
}

/// One record: its identity, its collection, and its values keyed by field id.
#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    /// Stable id within its collection; the last coordinate of a node id.
    pub id: String,
    /// The collection this record belongs to.
    pub collection: String,
    /// Whether the source has deleted it. A deleted record derives nothing at all.
    pub deleted: bool,
    /// Source-assigned version, a `u32` (D6: never `usize` on the wire).
    pub updated_at: u32,
    /// Its cells, keyed by field id, in no order of consequence (H6).
    pub values: Vec<(String, JsonValue)>,
}

impl Record {
    /// The value of `field_id`, or `None`. A lookup by declared id, never by position.
    pub fn value(&self, field_id: &str) -> Option<&JsonValue> {
        self.values
            .iter()
            .find(|(id, _)| id == field_id)
            .map(|(_, value)| value)
    }
}

/// A whole ingest document: the collections' declared roles, and the records.
#[derive(Debug, Clone, PartialEq)]
pub struct Ingest {
    /// The document's version; [`VERSION`] is the only one this reader accepts.
    pub version: u32,
    /// The backend the records came from; the first coordinate of every node id.
    pub source: String,
    /// The declared collections, in document order.
    pub collections: Vec<Collection>,
    /// The records, in document order. Order is the caller's; the derivation is
    /// order-independent by construction (it sorts where it must).
    pub records: Vec<Record>,
}

impl Ingest {
    /// The collection with id `id`, or `None`.
    pub fn collection(&self, id: &str) -> Option<&Collection> {
        self.collections.iter().find(|c| c.id == id)
    }
}

/// A cell value: the whole of JSON, nothing more, so a source's own types survive.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum JsonValue {
    /// `null`.
    Null,
    /// `true` or `false`.
    Bool(bool),
    /// A finite number. An overflowing literal is refused at read (D9).
    Number(f64),
    /// A string.
    Text(String),
    /// An array.
    List(Vec<JsonValue>),
    /// An object's members, keys unique (the reader refuses a repeat).
    Map(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// The value as text, or `None` if it is not a string. Never a lossy stringify: a
    /// number in a `title` field is a role mistake, not a title.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The value as a number, or `None`.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Self::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// The value as a list, or `None`. A single value is never read as a one-element
    /// list: `cardinality: "one"` and `"many"` are declared, and reading one as the
    /// other would hide a schema mistake.
    pub fn as_list(&self) -> Option<&[JsonValue]> {
        match self {
            Self::List(items) => Some(items),
            _ => None,
        }
    }
}
