//! One derived edge as a value, before it becomes an [`EdgeRecord`].
//!
//! Split out of `builder.rs` for the house's 300-line limit and for no other reason: the
//! walk is in the parent, and this is the one place that knows what an edge is called.

use crate::ingest::edge_strength as strength;
use crate::{EdgeIdParts, EdgeKind, EdgeRecord, make_edge_id};

/// An edge's two endpoints, as a value: the house limit is four parameters and a derived
/// edge is five facts, so the endpoints travel together rather than as a fifth and sixth
/// argument.
pub(super) struct Ends {
    source: String,
    target: String,
}

impl Ends {
    pub(super) fn new(source: String, target: String) -> Self {
        Self { source, target }
    }
}

/// One derived edge's five facts, owned (and owning the strings sidesteps a borrow of the
/// parent's `edges` that would otherwise have to outlive the push).
pub(super) struct Spec {
    ends: Ends,
    kind: EdgeKind,
    label: String,
    directed: bool,
}

impl Spec {
    pub(super) fn new(
        ends: Ends,
        kind: EdgeKind,
        label: impl Into<String>,
        directed: bool,
    ) -> Self {
        Self {
            ends,
            kind,
            label: label.into(),
            directed,
        }
    }

    /// The id this spec becomes, from the one place that decides it: [`Spec::edge`] reads
    /// the same [`parts`](Self::parts), so a duplicate check and the emitted edge can never
    /// disagree about what this edge is called.
    pub(super) fn id(&self) -> String {
        make_edge_id(&self.parts())
    }

    pub(super) fn edge(&self) -> EdgeRecord {
        EdgeRecord {
            id: self.id(),
            source: self.ends.source.clone(),
            target: self.ends.target.clone(),
            kind: self.kind,
            label: self.label.clone(),
            strength: strength(self.kind),
            directed: self.directed,
            record_id: None,
            child_first: false,
        }
    }

    fn parts(&self) -> EdgeIdParts<'_> {
        EdgeIdParts {
            source: &self.ends.source,
            target: &self.ends.target,
            kind: self.kind,
            label: &self.label,
            directed: self.directed,
        }
    }
}
