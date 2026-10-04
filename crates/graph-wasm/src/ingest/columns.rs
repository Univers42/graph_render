//! The columnar ingest path's bridge from a decoded document to graph-core's admit path
//! (`docs/contract/ingest-columns.md`, `docs/decisions/ingest-columns.md`).
//!
//! graph-core knows no wire format: it takes rows whose strings are entries of an
//! [`EntryTable`], and resolves each entry, kind names included, the first time a row names
//! it. This module is the whole of the translation — lend the table, copy each row's cells
//! across — and it lives here because graph-wasm is the one crate that depends on both
//! `graph-contract` (which owns the bytes) and `graph-core` (which owns the graph).

use graph_contract::ingest_columns::{self as wire, ColumnsDoc, decode_batch};
use graph_core::{BatchEdgeCells, EntryTable, NodeCells, Topology};

// `index` is `gm_build_columns`'s body and `edge` its row mapping, so both are compiled only
// where the exports are (C21): the native build reaches this module for `extend_batch`, which
// the service façade shares with the force gate and the bench.
#[cfg(any(test, target_arch = "wasm32"))]
use graph_contract::ingest_columns::decode;
#[cfg(any(test, target_arch = "wasm32"))]
use graph_core::{EdgeCells, index_columns};

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
///
/// Row `r` becomes dense index `r`, which is what an edge's endpoint rows name — and it only
/// holds because `index_columns` refuses a repeated id instead of dropping the row.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn index(bytes: &[u8]) -> Result<Topology, ColumnsError> {
    if bytes.len() > crate::ingest::MAX_INGEST_BYTES {
        return Err(ColumnsError::TooLarge);
    }
    let doc = decode(bytes).map_err(|_| ColumnsError::Invalid)?;
    let nodes =
        (0..doc.node_count()).map(|row| node(doc.node_cells(row).expect("a row below the count")));
    let edges =
        (0..doc.edge_count()).map(|row| edge(doc.edge_cells(row).expect("a row below the count")));
    index_columns(&Table(doc), nodes, edges).map_err(|_| ColumnsError::Invalid)
}

/// Appends the `GMX1` batch in `bytes` to `topology`: `gm_graph_extend_columns`'s body
/// (`docs/decisions/extend-columns.md`).
///
/// The same two refusals as a whole document's, and the same code for both classes — the
/// decoder refused the buffer, or the graph refused what it describes
/// ([`ColumnsError::Invalid`]). The length is checked first, before the decoder is handed a
/// byte, so a buffer over the ceiling is `IngestTooLarge` and never a malformed document —
/// `gm_graph_extend_columns` reaches this only after `InvalidHandle` and `BuildSourceInvalid`.
///
/// **Caveat:** the rows are handed over as an iterator of the decoded cells, read twice — once
/// to resolve, once to admit — so a batch pays no `Vec` of rows on the way in, only the
/// per-entry memo graph-core keeps for it.
pub fn extend_batch(topology: &mut Topology, bytes: &[u8]) -> Result<(), ColumnsError> {
    if bytes.len() > crate::ingest::MAX_INGEST_BYTES {
        return Err(ColumnsError::TooLarge);
    }
    let doc = decode_batch(bytes).map_err(|_| ColumnsError::Invalid)?;
    let nodes = || {
        (0..doc.node_count()).map(|row| node(doc.node_cells(row).expect("a row below the count")))
    };
    let edges = || {
        (0..doc.edge_count()).map(|row| {
            let c = doc.edge_cells(row).expect("a row below the count");
            // The two endpoints are entries naming node ids, which is the whole difference
            // between a batch's row and a document's: a batch's edge may name a node the
            // graph already holds.
            BatchEdgeCells {
                id: c.id,
                source_entry: c.source_row,
                target_entry: c.target_row,
                kind: c.kind,
                label: c.label,
                record_id: c.record_id,
                strength: c.strength,
                directed: c.directed,
                child_first: c.child_first,
            }
        })
    };
    topology
        .extend_columns(&Table(doc), nodes(), edges())
        .map_err(|_| ColumnsError::Invalid)
}

/// The document's string table, lent to graph-core. A newtype because graph-core owns the
/// trait and graph-contract the type, and only a local type may join them.
struct Table<'d>(ColumnsDoc<'d>);

impl EntryTable for Table<'_> {
    fn entries(&self) -> usize {
        self.0.string_count() as usize
    }

    fn bytes(&self) -> usize {
        self.0.blob().len()
    }

    fn text(&self, entry: u32) -> Option<&str> {
        self.0.text(entry)
    }
}

fn node(c: wire::NodeCells) -> NodeCells {
    NodeCells {
        id: c.id,
        kind: c.kind,
        database_id: c.database_id,
        source: c.source,
        label: c.label,
        group: c.group,
        weight: c.weight,
        version: c.version,
        has_note: c.has_note,
        icon: c.icon,
    }
}

#[cfg(any(test, target_arch = "wasm32"))]
fn edge(c: wire::EdgeCells) -> EdgeCells {
    EdgeCells {
        id: c.id,
        source_row: c.source_row,
        target_row: c.target_row,
        kind: c.kind,
        label: c.label,
        strength: c.strength,
        directed: c.directed,
        record_id: c.record_id,
        child_first: c.child_first,
    }
}
