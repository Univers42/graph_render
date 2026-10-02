//! Ingest to graph: **one** derivation, in the motor, for every source.
//!
//! This is the module the phase exists for. Graph derivation existed in three copies in
//! the host and the copies had already diverged, so two live code paths produced
//! different layouts for the same data — silently, because both returned a well-formed
//! graph. Here there is one function, [`build`], and it reads declared roles
//! ([`super::roles`]) rather than a host's property-type strings.
//!
//! ## What is derived
//!
//! | From | Nodes | Edges |
//! |---|---|---|
//! | one live record | one `record` node, id `source:collection:record` | — |
//! | the collection's `titleField` | the node's `label`, the record id when absent | — |
//! | the `label` role, else the `group` role | the node's `group` | — |
//! | the `weight` role | the node's `weight`, as declared | — |
//! | the record's `updatedAt` | the node's `version` | — |
//! | the `parent` role | — | one `hierarchy` edge, parent first |
//! | a `link` role | — | one `relation` edge per referenced record, labelled by **field id** |
//! | the `tags` role | one `tag` hub node per distinct value | one `tag` edge per value |
//! | the `scalar` role | nothing: declared and read by nobody | nothing |
//!
//! Two node columns are **not** derived from any role: `has_note` is always `false` and
//! `icon` always `null`. None of the eight roles carries a note body or an icon, so there
//! is no declaration to read them from and no value to invent.
//!
//! A tag hub's `version` is `0.0`, because a tag is not a record and carries no
//! `updatedAt`. It is told apart from a record whose `updatedAt` is `0` by `kind`, not by
//! the version column.
//!
//! A **deleted** record derives no node and no edge of its own — the alternative (a node
//! the user cannot see, still pulling a layout) is the direction that matters, so deletion
//! is honoured here rather than filtered downstream.
//!
//! An edge a *live* record draws **towards** a deleted or absent record is a different
//! question, and it is **stated** rather than dropped: `Derived.edges` is what the document
//! claimed, and `index_model` is where an edge naming no derived node goes — its pinned
//! rule is `edges_skip_taken_ids_and_dangling_endpoints_without_claiming_the_id`. So a
//! link to a deleted record yields exactly the graph the same document yields with that
//! cell removed, and nothing is silently reparented.
//!
//! ## Determinism
//!
//! Output order is the document's own, with tag hubs after the records in first-
//! appearance order — never a hash order (D4), never a `sort_unstable` (D5). The edges
//! of one record come out hierarchy, then relations, then tags. Edge strengths come
//! from [`super::edge_strength`], the one table, so this module cannot grow a second
//! convention, and edge ids come from `make_edge_id`, so a derived edge's identity is
//! the same identity the host's would be.
//!
//! # The id grammar (H5) — the constraint, enforced here
//!
//! The derivation builds `source:collection:record` ids, and it refuses a value that
//! cannot become the id it is about to become. The contract's reader already refuses a
//! `:` in `source` or a collection id; this module refuses a `:` in a **tag value** too,
//! which the reader does not: `tag:a:b` begins with the reserved `tag:` prefix and no
//! consumer following the same grammar can read the value back out of it. A **record
//! id** may contain `:` freely — it is the last segment, which is exactly what the
//! grammar's `splitn(3, ':')` is for, and a test pins the round trip.
//!
//! **Ponytail (a tag value containing `:`).** Failing input: a tag value like `"a:b"` —
//! legal JSON, legal in any source's vocabulary. Direction: the derivation **refuses**
//! the document rather than emitting `tag:a:b`, which reads as a tag whose value is
//! `a:b` under a naive split and as no value at all under this one; a silently wrong id
//! is the dangerous direction, and no encoding escape keeps the value readable.
//! Escape hatch: rename the value at the source, or declare the field `scalar` and
//! carry the colon in a node label instead.

use crate::{EdgeRecord, NodeRecord, Topology, index_model};
use graph_contract::ingest::Ingest;
use std::fmt::Write;

mod builder;
use builder::Builder;

/// The graph a document derives, in derivation order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Derived {
    /// One node per live record, then one per distinct tag value.
    pub nodes: Vec<NodeRecord>,
    /// The edges, in derivation order.
    pub edges: Vec<EdgeRecord>,
}

/// Why a document could not be derived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A record names a collection the document does not declare.
    UnknownCollection {
        /// The record's id.
        record: String,
        /// The collection it named.
        collection: String,
    },
    /// A `link` role names a collection the document does not declare.
    UnknownLinkTarget {
        /// The collection declaring the field.
        collection: String,
        /// The field's id.
        field: String,
        /// The collection it links to.
        target: String,
    },
    /// The same record id twice in one collection: refused, not first-wins.
    DuplicateRecord {
        /// The collection both records are in.
        ///
        /// Present because the id alone does not name a record: the same id in two
        /// collections is two records, and an error that cannot say which one was at
        /// fault is not actionable.
        collection: String,
        /// The record's id.
        record: String,
    },
    /// A value that cannot become the id it is about to become (H5).
    IdGrammar {
        /// Which coordinate, e.g. `tag value`.
        coordinate: &'static str,
        /// The offending text.
        value: String,
    },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCollection { record, collection } => {
                write!(f, "record `{record}`: no collection with id `{collection}`")
            }
            Self::UnknownLinkTarget {
                collection,
                field,
                target,
            } => write!(
                f,
                "field `{field}` of collection `{collection}`: links to collection \
                 `{target}`, which is not declared"
            ),
            Self::DuplicateRecord { collection, record } => write!(
                f,
                "record `{record}` of collection `{collection}`: declared twice"
            ),
            Self::IdGrammar { coordinate, value } => write!(
                f,
                "{coordinate} {value:?} contains `:` and cannot round-trip through the \
                 node-id grammar (H5): the id would parse back shifted"
            ),
        }
    }
}

/// Derives `doc` into nodes and edges, or the refusal.
///
/// Every refusal is a *document* fact rather than a per-record default: a dangling
/// collection, a link to nowhere, a repeated record, or a value that cannot become the
/// id it is about to become. Each would otherwise be a graph that is well-formed and
/// wrong.
pub fn build(doc: &Ingest) -> Result<Derived, BuildError> {
    let mut builder = Builder::new(doc);
    builder.check_targets()?;
    builder.run()?;
    Ok(Derived {
        nodes: builder.take_nodes(),
        edges: builder.take_edges(),
    })
}

/// The derived graph, indexed. The two steps are separate on purpose: [`build`] is the
/// derivation and indexing is the motor's own topology step with its own capacity
/// refusal, so a caller can see which of the two said no.
pub fn build_topology(doc: &Ingest) -> Result<(Derived, Topology), BuildError> {
    let derived = build(doc)?;
    // A derived graph's size is the document's own, and CapacityError here would be a
    // u32 exhaustion no document a person can hold reaches; the indexing call sites
    // that can refuse are the ones that read raw records.
    let topology = index_model(&derived.nodes, &derived.edges).expect("a derived graph indexes");
    Ok((derived, topology))
}

/// The derived graph as canonical JSON, the same rule the contract's writer uses:
/// compact, object keys sorted by bytes, arrays in derived order.
///
/// Not the snapshot and not a replacement for it — it is the **identity** of a
/// derivation, for the one thing a snapshot cannot do: exist before any layout has run.
/// The convergence test compares this text across two adapters' documents, and a diff
/// of two derivations is a diff of two of these.
pub fn to_canonical_json(derived: &Derived) -> String {
    graph_contract::ingest::to_json_value(&json_of(derived))
}

fn json_of(derived: &Derived) -> graph_contract::ingest::JsonValue {
    use graph_contract::ingest::JsonValue as J;
    let object =
        |members: Vec<(&str, J)>| J::Map(members.into_iter().map(|(k, v)| (k.into(), v)).collect());
    let nodes = derived.nodes.iter().map(|n| {
        object(vec![
            ("id", J::Text(n.id.clone())),
            ("kind", J::Text(n.kind.as_str().into())),
            ("database_id", optional(&n.database_id)),
            ("source", J::Text(n.source.clone())),
            ("label", J::Text(n.label.clone())),
            ("group", optional(&n.group)),
            ("weight", J::Number(n.weight)),
            ("version", J::Number(n.version)),
            ("has_note", J::Bool(n.has_note)),
            ("icon", optional(&n.icon)),
        ])
    });
    let edges = derived.edges.iter().map(|e| {
        object(vec![
            ("id", J::Text(e.id.clone())),
            ("source", J::Text(e.source.clone())),
            ("target", J::Text(e.target.clone())),
            ("kind", J::Text(e.kind.as_str().into())),
            ("label", J::Text(e.label.clone())),
            ("strength", J::Number(e.strength)),
            ("directed", J::Bool(e.directed)),
            ("record_id", optional(&e.record_id)),
            ("child_first", J::Bool(e.child_first)),
        ])
    });
    object(vec![
        ("nodes", J::List(nodes.collect())),
        ("edges", J::List(edges.collect())),
    ])
}

/// An absent optional is `null` in this text, not an empty string: the two are
/// different facts about a record, and a rendering that conflated them would hide a
/// missing group behind a blank one.
fn optional(value: &Option<String>) -> graph_contract::ingest::JsonValue {
    match value {
        Some(text) => graph_contract::ingest::JsonValue::Text(text.clone()),
        None => graph_contract::ingest::JsonValue::Null,
    }
}

/// A readable rendering of a derived graph, for a test or a diagnostic. Not a wire
/// format — the wire format is the snapshot. Line order is the derived order, so a diff
/// of two renderings is a diff of two derivations.
pub fn describe(derived: &Derived) -> String {
    let mut out = String::new();
    for node in &derived.nodes {
        let _ = writeln!(
            out,
            "node {} {:?} label={:?} group={:?} weight={} version={}",
            node.id, node.kind, node.label, node.group, node.weight, node.version
        );
    }
    for edge in &derived.edges {
        let _ = writeln!(
            out,
            "edge {} {} -> {} {:?} label={:?} strength={} directed={}",
            edge.id, edge.source, edge.target, edge.kind, edge.label, edge.strength, edge.directed
        );
    }
    out
}
