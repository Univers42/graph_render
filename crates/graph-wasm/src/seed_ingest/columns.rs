//! `GMX1`, the extend batch, written from the same records [`document`](super::document)
//! writes as JSON: [`columns_batch`] is the one writer the round-trip test, `tick --path
//! columns` and the force-gate's columns arm all go through, so a batch number and a gate
//! digest are timed over one encoder's bytes.
//!
//! It lives here rather than beside the decoder because the records are graph-core's
//! ([`NodeRecord`], [`EdgeRecord`]) and the sections are graph-contract's, and this is the
//! one crate that depends on both — the same reasoning [`crate::ingest::columns`] gives for
//! the reader. The section arithmetic is written out rather than shared with
//! `ingest_columns::layout` (private to that crate, and deliberately allocation-free), and
//! what holds the two together is the round-trip test: encode here, decode there, same
//! records.
//!
//! Unlike [`document`](super::document) this cannot fail. The columnar form carries a float
//! as its eight bytes, so a non-finite weight or strength is *written* and refused by the
//! decoder rather than refused here: one rule, and it is the decoder's.

use graph_contract::ingest_columns::{Format, VERSION};
use graph_core::{EdgeRecord, NodeRecord};
use std::collections::BTreeMap;

/// `u32::MAX` in an optional column: the contract's absent marker.
const ABSENT: u32 = u32::MAX;

/// The batch `nodes` and `edges` describe, as a `GMX1` columnar document
/// (`docs/contract/ingest-columns.md`): one UTF-8 string table, `u32` and `f64` columns, and
/// edge endpoints as string entries **naming node ids**.
///
/// An endpoint naming a node the graph already holds is interned here on demand, into the
/// same table: that is the one thing a batch's endpoints may do that a whole document's
/// cannot, and it is why this is not `document` again.
pub fn columns_batch(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Vec<u8> {
    let table = Table::new(nodes, edges);
    let marks = Marks::of(&table, nodes.len(), edges.len());
    let mut out = vec![0u8; marks.total];
    debug_assert_eq!(marks.total, out.len(), "the buffer is the declared total");
    table.write_header(&mut out, &marks, nodes.len(), edges.len());
    table.write_strings(&mut out, &marks);
    write_floats(&mut out, marks.columns, nodes, edges);
    // The `u32` columns follow the three `f64` ones: the reader walks the sections in
    // contract order, so their start is a function of the counts and nothing else.
    let narrow = marks.columns + 8 * (2 * nodes.len() + edges.len());
    write_nodes(&mut out, &table, nodes, narrow);
    write_edges(&mut out, &table, edges, narrow + 32 * nodes.len());
    out
}

/// The string table: entries interned by content, in first-seen order.
///
/// `BTreeMap`, not a hash map (H2, D4): interning is not the hot path — every measurement
/// that uses this writer times only what comes after it — and an ordered map makes the entry
/// order a property of the record order alone, so two writes of the same records are the
/// same bytes.
struct Table<'a> {
    texts: Vec<&'a str>,
    at: BTreeMap<&'a str, u32>,
}

impl<'a> Table<'a> {
    /// Walks the records once and interns every string any cell will name, endpoints
    /// included: an endpoint may name a node this batch does not carry, and a name that is
    /// not in the table is a document the decoder refuses.
    fn new(nodes: &'a [NodeRecord], edges: &'a [EdgeRecord]) -> Self {
        let mut table = Self {
            texts: Vec::new(),
            at: BTreeMap::new(),
        };
        for n in nodes {
            for text in [
                Some(n.id.as_str()),
                Some(n.kind.as_str()),
                n.database_id.as_deref(),
                Some(n.source.as_str()),
                Some(n.label.as_str()),
                n.group.as_deref(),
                n.icon.as_deref(),
            ] {
                table.intern(text);
            }
        }
        for e in edges {
            for text in [
                Some(e.id.as_str()),
                Some(e.source.as_str()),
                Some(e.target.as_str()),
                Some(e.kind.as_str()),
                Some(e.label.as_str()),
                e.record_id.as_deref(),
            ] {
                table.intern(text);
            }
        }
        table
    }

    /// Entry for `text`, interning it the first time a row names it. `None` is
    /// [`ABSENT`], legal only in an optional cell.
    fn intern(&mut self, text: Option<&'a str>) -> u32 {
        let Some(text) = text else {
            return ABSENT;
        };
        if let Some(&index) = self.at.get(text) {
            return index;
        }
        let index = self.len();
        self.texts.push(text);
        self.at.insert(text, index);
        index
    }

    /// Entry for a string [`new`](Self::new) already interned. A miss is this writer's own
    /// bug — a cell whose text the interning pass never saw — so it is named rather than
    /// written as an entry that is not there.
    fn entry(&self, text: &'a str) -> u32 {
        self.at
            .get(text)
            .copied()
            .unwrap_or_else(|| panic!("a cell naming a string the table never interned: {text:?}"))
    }

    /// [`entry`](Self::entry) for an optional cell.
    fn optional(&self, text: Option<&'a str>) -> u32 {
        text.map_or(ABSENT, |text| self.entry(text))
    }

    /// How many entries the table holds.
    fn len(&self) -> u32 {
        u32::try_from(self.texts.len()).expect("a batch under the ingest ceiling is below 2^32")
    }

    /// Every entry's bytes together.
    fn bytes(&self) -> usize {
        self.texts.iter().map(|text| text.len()).sum()
    }

    /// The eight header words: the magic that says *batch*, the version, the four counts and
    /// the two reserved words the contract requires to be zero.
    fn write_header(&self, out: &mut [u8], marks: &Marks, nodes: usize, edges: usize) {
        for (word, value) in [
            Format::Batch.magic(),
            VERSION,
            count(nodes),
            count(edges),
            self.len(),
            count(self.bytes()),
            0,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            put_u32(out, 4 * word, value);
        }
        debug_assert_eq!(marks.total, out.len());
    }

    /// The offsets table and the blob. The pad between them is already zero: the buffer is
    /// allocated as one run of zero bytes, and the decoder refuses a nonzero pad byte.
    fn write_strings(&self, out: &mut [u8], marks: &Marks) {
        let mut at = 0usize;
        let mut offsets = marks.offsets;
        for text in &self.texts {
            put_u32(out, offsets, at as u32);
            offsets += 4;
            let end = marks.blob + at + text.len();
            out[marks.blob + at..end].copy_from_slice(text.as_bytes());
            at += text.len();
        }
        put_u32(out, offsets, at as u32);
    }
}

/// Every section's byte offset and the buffer's total, from the counts alone.
///
/// `total` is exactly what the decoder's exact-size check demands of a document this writer
/// produced, which is why the buffer is allocated once at that size instead of grown: no
/// reallocation, and a length the reader accepts by construction.
struct Marks {
    /// The `string_count + 1` offsets.
    offsets: usize,
    /// The blob.
    blob: usize,
    /// The first column, eight-aligned from the buffer's start. The zero pad between the
    /// two is implied by it: `columns - (blob + bytes)`, which is where `of` measured from.
    columns: usize,
    /// The buffer's exact length.
    total: usize,
}

impl Marks {
    /// `docs/contract/ingest-columns.md`'s section table in the reader's own arithmetic:
    /// header, offsets, blob, pad, then three `f64` columns and sixteen `u32` ones.
    fn of(table: &Table<'_>, nodes: usize, edges: usize) -> Self {
        let offsets = 32;
        let blob = offsets + 4 * (table.texts.len() + 1);
        let end_of_blob = blob + table.bytes();
        // Whole, so the first column starts on an eight-byte boundary from the start of
        // the buffer — which is what lets the reader hand the `f64` columns back without a
        // pointer cast.
        let pad = (8 - end_of_blob % 8) % 8;
        let columns = end_of_blob + pad;
        // Two `f64` columns per node and one per edge; eight `u32` columns per record kind.
        let floats = 8 * (2 * nodes + edges);
        let narrow = 32 * (nodes + edges);
        Self {
            offsets,
            blob,
            columns,
            total: columns + floats + narrow,
        }
    }
}

/// `node weight`, `node version`, `edge strength`: the three `f64` columns, in that order.
fn write_floats(out: &mut [u8], at: usize, nodes: &[NodeRecord], edges: &[EdgeRecord]) {
    f64_column(out, at, nodes.len(), |i| nodes[i].weight);
    f64_column(out, at + 8 * nodes.len(), nodes.len(), |i| nodes[i].version);
    f64_column(out, at + 16 * nodes.len(), edges.len(), |i| {
        edges[i].strength
    });
}

/// The eight `u32` node columns, in contract order, each `4 * rows` long and adjacent to the
/// next: structure of arrays, so a row's fields are never neighbours.
fn write_nodes(out: &mut [u8], table: &Table<'_>, nodes: &[NodeRecord], at: usize) -> usize {
    let step = 4 * nodes.len();
    u32_column(out, at, nodes, |n| table.entry(&n.id));
    u32_column(out, at + step, nodes, |n| table.entry(n.kind.as_str()));
    u32_column(out, at + 2 * step, nodes, |n| {
        table.optional(n.database_id.as_deref())
    });
    u32_column(out, at + 3 * step, nodes, |n| table.entry(&n.source));
    u32_column(out, at + 4 * step, nodes, |n| table.entry(&n.label));
    u32_column(out, at + 5 * step, nodes, |n| {
        table.optional(n.group.as_deref())
    });
    u32_column(out, at + 6 * step, nodes, |n| {
        table.optional(n.icon.as_deref())
    });
    u32_column(out, at + 7 * step, nodes, |n| u32::from(n.has_note));
    at + 8 * step
}

/// The eight `u32` edge columns. The two endpoints are **entries naming node ids**: the whole
/// difference between this format and a document's dense rows, and the reason an endpoint
/// here may name a node the graph already holds.
fn write_edges(out: &mut [u8], table: &Table<'_>, edges: &[EdgeRecord], at: usize) {
    let step = 4 * edges.len();
    u32_column(out, at, edges, |e| table.entry(&e.id));
    u32_column(out, at + step, edges, |e| table.entry(&e.source));
    u32_column(out, at + 2 * step, edges, |e| table.entry(&e.target));
    u32_column(out, at + 3 * step, edges, |e| table.entry(e.kind.as_str()));
    u32_column(out, at + 4 * step, edges, |e| table.entry(&e.label));
    u32_column(out, at + 5 * step, edges, |e| {
        table.optional(e.record_id.as_deref())
    });
    u32_column(out, at + 6 * step, edges, |e| u32::from(e.directed));
    u32_column(out, at + 7 * step, edges, |e| u32::from(e.child_first));
}

/// One `f64` column of `rows` cells at `at`, row `i` read by `get`.
fn f64_column(out: &mut [u8], at: usize, rows: usize, get: impl Fn(usize) -> f64) {
    for row in 0..rows {
        put_f64(out, at + 8 * row, get(row));
    }
}

/// One `u32` column of `records.len()` cells at `at`.
fn u32_column<T>(out: &mut [u8], at: usize, records: &[T], get: impl Fn(&T) -> u32) {
    for (row, record) in records.iter().enumerate() {
        put_u32(out, at + 4 * row, get(record));
    }
}

/// A count a `u32` header word holds. A batch over the ingest ceiling is refused by the
/// export before it is decoded, and one under it cannot hold `2^32` rows.
fn count(value: usize) -> u32 {
    u32::try_from(value).expect("a count the ceiling keeps below 2^32")
}

fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_f64(out: &mut [u8], at: usize, value: f64) {
    out[at..at + 8].copy_from_slice(&value.to_bits().to_le_bytes());
}
