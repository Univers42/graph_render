//! The columnar ingest document (`docs/contract/ingest-columns.md`): one UTF-8 string
//! table, columns of `u32` and `f64`, and edge endpoints given as node row numbers.
//!
//! [`decode`] borrows the caller's buffer and hands back a [`ColumnsDoc`] that lends `&str`
//! and scalars from it. Nothing is copied, nothing is allocated, there is no `unsafe` and no
//! new dependency — the whole point is that the bytes arrive already shaped, so the read is
//! a walk of section lengths rather than a parse.
//!
//! Every refusal is one [`ColumnsError`] variant and has its own test in
//! [`tests`](self::tests). The ABI collapses them all into `Code::ColumnsInvalid`; the
//! variants are here so a test can say which rule it broke.

mod check;
mod error;
mod layout;

pub use error::ColumnsError;
pub use row::{EdgeCells, NodeCells};

use check::cell_at;

/// The nineteen columns, each borrowed, in the order the contract lists them. Structure of
/// arrays: row `r` of a column is one `u32` at `4 * r`, so reading a row touches one cache
/// line per column and never a struct.
#[derive(Debug, Clone, Copy)]
pub(super) struct Columns<'a> {
    /// Node weights.
    pub weight: &'a [u8],
    /// Node versions.
    pub version: &'a [u8],
    /// Edge strengths.
    pub strength: &'a [u8],
    /// Node ids, string indices.
    pub id: &'a [u8],
    /// Node kinds, string indices.
    pub kind: &'a [u8],
    /// Node database ids, string indices or [`ABSENT`](layout::ABSENT).
    pub database: &'a [u8],
    /// Node sources, string indices.
    pub source: &'a [u8],
    /// Node labels, string indices.
    pub label: &'a [u8],
    /// Node groups, string indices or [`ABSENT`](layout::ABSENT).
    pub group: &'a [u8],
    /// Node icons, string indices or [`ABSENT`](layout::ABSENT).
    pub icon: &'a [u8],
    /// Node note flags, `0` or `1`.
    pub has_note: &'a [u8],
    /// Edge ids, string indices.
    pub edge_id: &'a [u8],
    /// Edge source node rows.
    pub edge_source: &'a [u8],
    /// Edge target node rows.
    pub edge_target: &'a [u8],
    /// Edge kinds, string indices.
    pub edge_kind: &'a [u8],
    /// Edge labels, string indices.
    pub edge_label: &'a [u8],
    /// Edge backing row ids, string indices or [`ABSENT`](layout::ABSENT).
    pub record_id: &'a [u8],
    /// Edge directed flags, `0` or `1`.
    pub directed: &'a [u8],
    /// Edge child-first flags, `0` or `1`.
    pub child_first: &'a [u8],
}

/// One node row, borrowed: every field of a node, with the kind still a string because
/// `NodeKind::from_name` is graph-core's vocabulary and this crate does not have the enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeRow<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Kind name, to resolve with `NodeKind::from_name`.
    pub kind: &'a str,
    /// Source database, absent where the document said `u32::MAX`.
    pub database_id: Option<&'a str>,
    /// Owning backend.
    pub source: &'a str,
    /// Label.
    pub label: &'a str,
    /// Secondary label, absent where the document said `u32::MAX`.
    pub group: Option<&'a str>,
    /// Visual weight.
    pub weight: f64,
    /// Version.
    pub version: f64,
    /// Note overlay flag.
    pub has_note: bool,
    /// Icon, absent where the document said `u32::MAX`.
    pub icon: Option<&'a str>,
}

/// One edge row, borrowed, with endpoints as node row numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeRow<'a> {
    /// Content-addressed id.
    pub id: &'a str,
    /// Dense index of the source node.
    pub source_row: u32,
    /// Dense index of the target node.
    pub target_row: u32,
    /// Kind name, to resolve with `EdgeKind::from_name`.
    pub kind: &'a str,
    /// Label.
    pub label: &'a str,
    /// Strength.
    pub strength: f64,
    /// Directed flag.
    pub directed: bool,
    /// Backing row id, absent where the document said `u32::MAX`.
    pub record_id: Option<&'a str>,
    /// `source_row` is the child (`child_of`).
    pub child_first: bool,
}

/// A decoded columnar document, borrowing its buffer for as long as the host holds it.
///
/// Holding a `ColumnsDoc` means [`decode`] succeeded, so every string index in it is below
/// `string_count`, every endpoint row is below `node_count`, every boolean is `0` or `1` and
/// every float is finite. That is what lets [`node`](Self::node) and [`edge`](Self::edge) be
/// total for any row below the counts: the one input they cannot survive is a row number past
/// them, and those return `None` rather than panicking.
#[derive(Debug, Clone, Copy)]
pub struct ColumnsDoc<'a> {
    blob: &'a str,
    offsets: &'a [u8],
    columns: Columns<'a>,
    nodes: u32,
    edges: u32,
    strings: u32,
}

/// Reads a columnar ingest document. Nothing is allocated and nothing is copied: the
/// returned [`ColumnsDoc`] borrows `bytes` for as long as it is held.
///
/// The order of the checks is the order of the contract: header, then the exact total size
/// (both a short and a long buffer), then the offset table and the blob, then every value.
/// The size check comes before any slice is formed, which is what makes "no allocation sized
/// from a header field" true for a header that lies.
pub fn decode(bytes: &[u8]) -> Result<ColumnsDoc<'_>, ColumnsError> {
    let shape = layout::header(bytes)?;
    let layout = layout::layout(bytes, shape)?;
    check::check_table(layout.offsets, layout.blob, shape.strings as u32)?;
    check::check(&layout.columns, shape)?;
    Ok(ColumnsDoc {
        blob: layout.blob,
        offsets: layout.offsets,
        columns: layout.columns,
        nodes: shape.nodes as u32,
        edges: shape.edges as u32,
        strings: shape.strings as u32,
    })
}

impl<'a> ColumnsDoc<'a> {
    /// Node rows.
    pub fn node_count(&self) -> u32 {
        self.nodes
    }

    /// Edge rows.
    pub fn edge_count(&self) -> u32 {
        self.edges
    }

    /// String-table entries.
    pub fn string_count(&self) -> u32 {
        self.strings
    }

    /// The blob, one borrow of the whole table's bytes.
    pub fn blob(&self) -> &'a str {
        self.blob
    }

    /// Entry `index` of the string table, or `None` if there is no such entry. The bounds
    /// check is on the offsets table, not on `blob`: `index == string_count` would read the
    /// closing offset and one past it, and the closing offset is a real entry's end, not a
    /// start.
    pub fn text(&self, index: u32) -> Option<&'a str> {
        let at = index as usize;
        let lo = cell_at(self.offsets, at)?;
        let hi = cell_at(self.offsets, at.checked_add(1)?)?;
        self.blob.get(lo as usize..hi as usize)
    }
}

mod row;

#[cfg(test)]
mod tests;
