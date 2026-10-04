//! The differential corpus: every field class the contract can carry, in documents small
//! enough to read.
//!
//! The JSON path is the reference, so a generated document is *records*, not text: it is
//! written as JSON by the caller, read back by `read_records`, and encoded to columns from
//! the records the same reader produced. That way a disagreement can only be in the columnar
//! path — never in a second JSON spelling of the same graph.

use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

/// One document of the corpus: the JSON the reference path reads, and the records the
/// columnar path is built from.
pub struct Document {
    /// A name, so a failure says which document.
    pub name: &'static str,
    /// The provisional-ingest JSON.
    pub json: String,
    /// The records, in the order the JSON lists them.
    pub nodes: Vec<NodeRecord>,
    /// The records, in the order the JSON lists them.
    pub edges: Vec<EdgeRecord>,
}

/// Every field class the decision record names, asserted present by
/// `the_corpus_covers_every_field_class`.
///
/// The list is the contract's own list: optional and non-optional on both sides, every kind
/// on both sides, and the four float and boolean extremes that a naive encoder loses.
#[derive(Debug, Clone, Copy, Default)]
pub struct Coverage {
    /// An edge with a `record_id`.
    pub record_id: bool,
    /// A node with a `database_id`.
    pub database_id: bool,
    /// A node with no `database_id`.
    pub no_database_id: bool,
    /// A node with a `group`.
    pub group: bool,
    /// A node with no `group`.
    pub no_group: bool,
    /// A node with an `icon`.
    pub icon: bool,
    /// A node with no `icon`.
    pub no_icon: bool,
    /// A node of every [`NodeKind`].
    pub node_kinds: [bool; 4],
    /// An edge of every [`EdgeKind`].
    pub edge_kinds: [bool; 5],
    /// A nonzero `version`.
    pub version: bool,
    /// A negative `weight`.
    pub negative_weight: bool,
    /// A `-0.0` weight.
    pub negative_zero: bool,
    /// A subnormal `weight` or `strength`.
    pub subnormal: bool,
    /// An edge with `child_first` true.
    pub child_first: bool,
    /// An edge with `directed` false.
    pub not_directed: bool,
    /// An id outside ASCII.
    pub multi_byte_id: bool,
    /// A node with `has_note` true.
    pub has_note: bool,
    /// An empty string as a label.
    pub empty_label: bool,
}

impl Coverage {
    /// Folds one document in.
    pub fn add(&mut self, doc: &Document) {
        for node in &doc.nodes {
            self.database_id |= node.database_id.is_some();
            self.no_database_id |= node.database_id.is_none();
            self.group |= node.group.is_some();
            self.no_group |= node.group.is_none();
            self.icon |= node.icon.is_some();
            self.no_icon |= node.icon.is_none();
            self.has_note |= node.has_note;
            self.version |= node.version != 0.0;
            self.negative_weight |= node.weight < 0.0;
            self.negative_zero |= node.weight == 0.0 && node.weight.is_sign_negative();
            self.subnormal |= node.weight.is_subnormal() && node.weight != 0.0;
            self.multi_byte_id |= !node.id.is_ascii();
            self.empty_label |= node.label.is_empty();
            let at = NodeKind::ALL
                .iter()
                .position(|k| *k == node.kind)
                .unwrap_or(0);
            self.node_kinds[at] = true;
        }
        for edge in &doc.edges {
            self.record_id |= edge.record_id.is_some();
            self.child_first |= edge.child_first;
            self.not_directed |= !edge.directed;
            self.subnormal |= edge.strength.is_subnormal() && edge.strength != 0.0;
            let at = EdgeKind::ALL
                .iter()
                .position(|k| *k == edge.kind)
                .unwrap_or(0);
            self.edge_kinds[at] = true;
        }
    }

    /// Every class in the decision record's list.
    pub fn complete(&self) -> bool {
        self.record_id
            && self.database_id
            && self.no_database_id
            && self.group
            && self.no_group
            && self.icon
            && self.no_icon
            && self.node_kinds.iter().all(|seen| *seen)
            && self.edge_kinds.iter().all(|seen| *seen)
            && self.version
            && self.negative_weight
            && self.negative_zero
            && self.subnormal
            && self.child_first
            && self.not_directed
            && self.multi_byte_id
            && self.has_note
            && self.empty_label
    }
}
