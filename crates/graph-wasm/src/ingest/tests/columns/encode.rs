//! The test encoder: `NodeRecord`/`EdgeRecord` slices to the columnar document
//! (`docs/contract/ingest-columns.md`).
//!
//! It exists so the differential compares the *same* records through both paths, with no
//! second implementation of the JSON in the test. It is not the shipping encoder — that is
//! `crates/graph-sdk-js/src/columns.ts` — but it is the Rust side of the contract and the
//! differential's own half of the round trip.
//!
//! Strings are interned through a [`BTreeMap`], never a [`HashMap`]: a hash map's iteration
//! order would put the table's entry order at the mercy of the hasher, and that order is
//! part of the bytes (D4).

use std::collections::BTreeMap;

use graph_core::{EdgeRecord, NodeRecord};

/// `u32::MAX` in an optional column.
const ABSENT: u32 = u32::MAX;
/// `0x31434D47`: `"GMC1"` as a little-endian `u32`.
const MAGIC: u32 = 0x3143_4D47;
/// The header is eight `u32` words.
const HEADER: usize = 32;

/// The document, plus the two section starts a mutation needs to patch one cell.
pub struct Encoded {
    /// The bytes.
    pub bytes: Vec<u8>,
    /// Byte offset of the `edge source` column.
    pub edge_source: usize,
    /// Byte offset of the `edge child_first` column.
    pub edge_child_first: usize,
}

/// The string table, interned in first-seen order.
#[derive(Default)]
struct Table {
    strings: Vec<String>,
    index: BTreeMap<String, u32>,
}

impl Table {
    fn intern(&mut self, value: &str) -> u32 {
        if let Some(&at) = self.index.get(value) {
            return at;
        }
        let at = u32::try_from(self.strings.len()).expect("a table below u32::MAX");
        self.strings.push(value.to_owned());
        self.index.insert(value.to_owned(), at);
        at
    }

    fn optional(&mut self, value: Option<&str>) -> u32 {
        value.map_or(ABSENT, |v| self.intern(v))
    }
}

/// One column of the document: `f64` for the three float columns, `u32` for the rest.
enum Column {
    /// A float column, one entry per row.
    Float(Vec<f64>),
    /// An integer column, one entry per row.
    Int(Vec<u32>),
}

impl Column {
    fn bytes(&self) -> usize {
        match self {
            Self::Float(values) => 8 * values.len(),
            Self::Int(values) => 4 * values.len(),
        }
    }

    fn write(&self, out: &mut [u8], at: usize) -> usize {
        match self {
            Self::Float(values) => {
                for (row, value) in values.iter().enumerate() {
                    let here = at + 8 * row;
                    out[here..here + 8].copy_from_slice(&value.to_bits().to_le_bytes());
                }
                at + 8 * values.len()
            }
            Self::Int(values) => {
                for (row, value) in values.iter().enumerate() {
                    let here = at + 4 * row;
                    out[here..here + 4].copy_from_slice(&value.to_le_bytes());
                }
                at + 4 * values.len()
            }
        }
    }
}

/// `nodes` and `edges` as a columnar document, in slice order.
///
/// # Panics
///
/// If a node id repeats, or an edge names an endpoint that is not in `nodes`: a test
/// encoder has no way to report a refusal. A corpus that needs a duplicate is written as a
/// mutation of the bytes, which is what the negative control does.
pub fn encode(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Encoded {
    let mut table = Table::default();
    let rows = node_rows(nodes);
    let columns = columns(nodes, edges, &rows, &mut table);
    assemble(nodes.len(), edges.len(), table, columns)
}

/// The node ids' rows: slice position, which is exactly the document's row number.
fn node_rows(nodes: &[NodeRecord]) -> BTreeMap<String, u32> {
    nodes
        .iter()
        .enumerate()
        .map(|(row, node)| (node.id.clone(), row as u32))
        .collect()
}

/// The nineteen columns, in contract order, with every string interned on the way.
fn columns(
    nodes: &[NodeRecord],
    edges: &[EdgeRecord],
    rows: &BTreeMap<String, u32>,
    table: &mut Table,
) -> Vec<Column> {
    let mut node_columns: Vec<Vec<u32>> = vec![Vec::with_capacity(nodes.len()); 8];
    for node in nodes {
        let cells = [
            table.intern(&node.id),
            table.intern(node.kind.as_str()),
            table.optional(node.database_id.as_deref()),
            table.intern(&node.source),
            table.intern(&node.label),
            table.optional(node.group.as_deref()),
            table.optional(node.icon.as_deref()),
            u32::from(node.has_note),
        ];
        for (column, value) in node_columns.iter_mut().zip(cells) {
            column.push(value);
        }
    }
    let mut edge_columns: Vec<Vec<u32>> = vec![Vec::with_capacity(edges.len()); 8];
    for edge in edges {
        let cells = [
            table.intern(&edge.id),
            *rows.get(edge.source.as_str()).expect("a source node exists"),
            *rows.get(edge.target.as_str()).expect("a target node exists"),
            table.intern(edge.kind.as_str()),
            table.intern(&edge.label),
            table.optional(edge.record_id.as_deref()),
            u32::from(edge.directed),
            u32::from(edge.child_first),
        ];
        for (column, value) in edge_columns.iter_mut().zip(cells) {
            column.push(value);
        }
    }
    let mut out: Vec<Column> = Vec::with_capacity(19);
    out.push(Column::Float(nodes.iter().map(|n| n.weight).collect()));
    out.push(Column::Float(nodes.iter().map(|n| n.version).collect()));
    out.push(Column::Float(edges.iter().map(|e| e.strength).collect()));
    out.extend(node_columns.into_iter().map(Column::Int));
    out.extend(edge_columns.into_iter().map(Column::Int));
    out
}

/// The header, the offsets table, the blob, the pad and every column, in that order.
fn assemble(nodes: usize, edges: usize, table: Table, columns: Vec<Column>) -> Encoded {
    let offsets = offsets(&table);
    let blob = table.strings.concat();
    let mut at = HEADER + 4 * offsets.len() + blob.len();
    at += (8 - at % 8) % 8;
    let mut marks = Vec::with_capacity(columns.len());
    let mut cursor = at;
    for column in &columns {
        marks.push(cursor);
        cursor += column.bytes();
    }
    let mut out = vec![0u8; cursor];
    let words = [
        MAGIC,
        1,
        nodes as u32,
        edges as u32,
        table.strings.len() as u32,
        blob.len() as u32,
        0,
        0,
    ];
    for (word, value) in words.into_iter().enumerate() {
        put_u32(&mut out, 4 * word, value);
    }
    for (i, offset) in offsets.iter().enumerate() {
        put_u32(&mut out, HEADER + 4 * i, *offset);
    }
    let blob_at = HEADER + 4 * offsets.len();
    out[blob_at..blob_at + blob.len()].copy_from_slice(blob.as_bytes());
    for (column, at) in columns.iter().zip(&marks) {
        column.write(&mut out, *at);
    }
    Encoded {
        bytes: out,
        edge_source: marks[11],
        edge_child_first: marks[18],
    }
}

/// The offsets table: `string_count + 1` entries, the last one the blob's length.
fn offsets(table: &Table) -> Vec<u32> {
    let mut at = 0;
    let mut out = vec![0];
    for value in &table.strings {
        at += u32::try_from(value.len()).expect("a string below u32::MAX");
        out.push(at);
    }
    out
}

fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
