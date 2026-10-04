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

/// Which of the two columnar documents a buffer holds: a whole graph, or one extend batch.
///
/// One enum rather than two readers, because the two documents differ in exactly one rule —
/// what an edge endpoint names — and a second `check_*.rs` beside [`check`] is how the two
/// would drift. It is threaded through [`layout`], [`check`] and [`row`], and it is decided
/// once, by the magic word ([`Format::magic`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `"GMC1"`: a whole document. An edge endpoint is a **node row number**, so a row that
    /// is dropped renumbers every row after it and repoints every edge that follows — which
    /// is why a repeated id is a refusal here and not a drop.
    Document,
    /// `"GMX1"`: one batch to append to a live graph (`docs/decisions/extend-columns.md`).
    /// An edge endpoint is a **string index naming a node id**, required: the row it means
    /// may belong to the graph already, and a dense row never crosses the wire.
    Batch,
}

impl Format {
    /// The magic word a buffer of this format starts with, as a little-endian `u32`.
    pub(super) fn magic(self) -> u32 {
        match self {
            Self::Document => layout::MAGIC,
            Self::Batch => layout::BATCH_MAGIC,
        }
    }
}

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
    /// Edge source endpoints: node rows under [`Format::Document`], string entries naming a
    /// node id under [`Format::Batch`].
    pub edge_source: &'a [u8],
    /// Edge target endpoints, in the same two senses as [`edge_source`](Self::edge_source).
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

/// One edge row, borrowed, with endpoints as node row numbers ([`Format::Document`]) or as
/// string entries naming node ids ([`Format::Batch`]); which one is [`ColumnsDoc::format`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeRow<'a> {
    /// Content-addressed id.
    pub id: &'a str,
    /// Dense index of the source node, or the entry its id is in.
    pub source_row: u32,
    /// Dense index of the target node, or the entry its id is in.
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
    format: Format,
}

/// Reads a columnar **document** (`"GMC1"`). Nothing is allocated and nothing is copied: the
/// returned [`ColumnsDoc`] borrows `bytes` for as long as it is held.
///
/// The order of the checks is the order of the contract: header, then the exact total size
/// (both a short and a long buffer), then the offset table and the blob, then every value.
/// The size check comes before any slice is formed, which is what makes "no allocation sized
/// from a header field" true for a header that lies.
pub fn decode(bytes: &[u8]) -> Result<ColumnsDoc<'_>, ColumnsError> {
    read(bytes, Format::Document)
}

/// Reads a columnar **batch** (`"GMX1"`, [`Format::Batch`]): the same sections and the same
/// order as [`decode`], read with one rule changed — an edge endpoint is a required string
/// index naming a node id rather than a node row. Same borrowing, same allocation-free walk.
///
/// A `"GMC1"` document is refused as [`ColumnsError::BadMagic`] here, and a `"GMX1"` batch is
/// refused the same way by [`decode`]: each reader refuses the other's bytes rather than
/// guessing which was meant.
pub fn decode_batch(bytes: &[u8]) -> Result<ColumnsDoc<'_>, ColumnsError> {
    read(bytes, Format::Batch)
}

/// One walk for both formats, the header word deciding which. Every rule below this line is
/// shared, and the format reaches the two checks that care about it as [`Shape::format`].
fn read(bytes: &[u8], format: Format) -> Result<ColumnsDoc<'_>, ColumnsError> {
    let shape = layout::header(bytes, format)?;
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
        format,
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

    /// Which document this is, decided by the magic word and fixed for the whole decode: it
    /// is what [`edge`](Self::edge)'s two endpoint cells mean.
    pub fn format(&self) -> Format {
        self.format
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
