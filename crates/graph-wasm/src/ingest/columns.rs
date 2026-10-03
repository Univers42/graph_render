//! The columnar ingest path's bridge from a decoded document to graph-core's admit path
//! (`docs/contract/ingest-columns.md`, `docs/decisions/ingest-columns.md`).
//!
//! graph-core knows no wire format: it takes borrowed `&str`, a resolved kind, and endpoint
//! *rows*. This module is the whole of the translation — resolve the two kinds, lend the
//! strings, hand the rows through — and it lives here because graph-wasm is the one crate
//! that depends on both `graph-contract` (which owns the bytes) and `graph-core` (which owns
//! the graph).
//!
//! **Caveat:** the kinds are resolved in a pass of their own, before the admit pass, because
//! `Iterator::next` cannot refuse and an unknown kind has to be a refusal rather than a
//! silently short document. That pass is a string compare per row and no allocation; it is
//! paid once and buys the admit pass a `NodeKind` per row with no `Option` in the loop.

use graph_contract::ingest_columns::{ColumnsDoc, EdgeRow, decode};
use graph_core::{EdgeKind, NodeKind, NodeView, RowEdge, Topology, index_columns};

use crate::errors::Code;

/// Why a columnar document was refused, as the ABI reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnsError {
    /// The buffer is longer than [`crate::ingest::MAX_INGEST_BYTES`], refused on its length
    /// before a byte of it is read (F-16). Never `ColumnsInvalid`: this document is not
    /// malformed, it is too big to hold.
    TooLarge,
    /// The document failed the contract, or the graph it describes cannot be indexed.
    Invalid,
}

impl ColumnsError {
    /// The wire code for this refusal.
    pub fn code(self) -> Code {
        match self {
            Self::TooLarge => Code::IngestTooLarge,
            Self::Invalid => Code::ColumnsInvalid,
        }
    }
}

/// Reads a columnar document and indexes it, or the refusal.
pub fn index(bytes: &[u8]) -> Result<Topology, ColumnsError> {
    if bytes.len() > crate::ingest::MAX_INGEST_BYTES {
        return Err(ColumnsError::TooLarge);
    }
    let doc = decode(bytes).map_err(|_| ColumnsError::Invalid)?;
    resolve_kinds(&doc)?;
    index_columns(Nodes::new(doc), Edges::new(doc)).map_err(|_| ColumnsError::Invalid)
}

/// Every node kind and edge kind in the document, or the refusal. The contract carries kinds
/// as string-table entries precisely so the enum needs no numeric mirror here; this is the
/// one place that vocabulary is applied.
fn resolve_kinds(doc: &ColumnsDoc<'_>) -> Result<(), ColumnsError> {
    for row in 0..doc.node_count() {
        if NodeKind::from_name(doc.node(row).expect("a row below the count").kind).is_none() {
            return Err(ColumnsError::Invalid);
        }
    }
    for row in 0..doc.edge_count() {
        if EdgeKind::from_name(doc.edge(row).expect("a row below the count").kind).is_none() {
            return Err(ColumnsError::Invalid);
        }
    }
    Ok(())
}

/// Node rows `0..node_count`, in document order. Row `r` becomes dense index `r`, which is
/// what the endpoint rows on [`Edges`] name — and it only holds because `index_columns`
/// refuses a repeated id instead of dropping the row that repeats it.
struct Nodes<'d> {
    doc: ColumnsDoc<'d>,
    next: u32,
}

impl<'d> Nodes<'d> {
    fn new(doc: ColumnsDoc<'d>) -> Self {
        Self { doc, next: 0 }
    }

    fn view(&self, row: u32) -> NodeView<'d> {
        let n = self.doc.node(row).expect("a row below the count");
        NodeView {
            id: n.id,
            kind: NodeKind::from_name(n.kind).expect("`resolve_kinds` resolved every kind"),
            database_id: n.database_id,
            source: n.source,
            label: n.label,
            group: n.group,
            weight: n.weight,
            version: n.version,
            has_note: n.has_note,
            icon: n.icon,
        }
    }
}

impl<'d> Iterator for Nodes<'d> {
    type Item = NodeView<'d>;

    fn next(&mut self) -> Option<NodeView<'d>> {
        let row = self.next;
        if row >= self.doc.node_count() {
            return None;
        }
        self.next += 1;
        Some(self.view(row))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

impl ExactSizeIterator for Nodes<'_> {
    fn len(&self) -> usize {
        (self.doc.node_count() - self.next) as usize
    }
}

/// Edge rows `0..edge_count`, in document order.
struct Edges<'d> {
    doc: ColumnsDoc<'d>,
    next: u32,
}

impl<'d> Edges<'d> {
    fn new(doc: ColumnsDoc<'d>) -> Self {
        Self { doc, next: 0 }
    }

    fn row(&self, row: u32) -> RowEdge<'d> {
        let e: EdgeRow<'d> = self.doc.edge(row).expect("a row below the count");
        RowEdge {
            id: e.id,
            source_row: e.source_row,
            target_row: e.target_row,
            kind: EdgeKind::from_name(e.kind).expect("`resolve_kinds` resolved every kind"),
            label: e.label,
            strength: e.strength,
            directed: e.directed,
            record_id: e.record_id,
            child_first: e.child_first,
        }
    }
}

impl<'d> Iterator for Edges<'d> {
    type Item = RowEdge<'d>;

    fn next(&mut self) -> Option<RowEdge<'d>> {
        let row = self.next;
        if row >= self.doc.edge_count() {
            return None;
        }
        self.next += 1;
        Some(self.row(row))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }
}

impl ExactSizeIterator for Edges<'_> {
    fn len(&self) -> usize {
        (self.doc.edge_count() - self.next) as usize
    }
}
