use super::*;
use std::collections::BTreeMap;

/// `u32::MAX` in an optional column: the encoder's "absent".
const ABSENT: u32 = u32::MAX;

/// One node row's cells, before they are written. Every optional starts absent so a test
/// only has to name the fields it cares about.
#[derive(Clone, Copy)]
pub(super) struct NodeCells {
    /// `weight`.
    pub weight: f64,
    /// `version`.
    pub version: f64,
    /// id string index.
    pub id: u32,
    /// kind string index.
    pub kind: u32,
    /// database string index.
    pub database: u32,
    /// source string index.
    pub source: u32,
    /// label string index.
    pub label: u32,
    /// group string index.
    pub group: u32,
    /// icon string index.
    pub icon: u32,
    /// `0` or `1`.
    pub has_note: u32,
}

/// One edge row's cells.
#[derive(Clone, Copy)]
pub(super) struct EdgeCells {
    /// id string index.
    pub id: u32,
    /// source node row.
    pub source: u32,
    /// target node row.
    pub target: u32,
    /// kind string index.
    pub kind: u32,
    /// label string index.
    pub label: u32,
    /// record id string index.
    pub record_id: u32,
    /// `0` or `1`.
    pub directed: u32,
    /// `0` or `1`.
    pub child_first: u32,
    /// `strength`.
    pub strength: f64,
}

/// Where each section starts, so a negative test patches one cell instead of rebuilding the
/// whole document. Byte offsets into [`Encoded::bytes`].
#[derive(Debug, Clone, Copy)]
pub(super) struct Marks {
    /// The offsets table.
    pub offsets: usize,
    /// The blob.
    pub blob: usize,
    /// The first pad byte; its length is `(-marks.pad) % 8` from the buffer start.
    pub pad: usize,
    /// `node weight`.
    pub weight: usize,
    /// `node version`.
    pub version: usize,
    /// `edge strength`.
    pub strength: usize,
    /// `node id`.
    pub node_id: usize,
    /// `node kind`.
    pub node_kind: usize,
    /// `node database`.
    pub node_database: usize,
    /// `node source`.
    pub node_source: usize,
    /// `node label`.
    pub node_label: usize,
    /// `node group`.
    pub node_group: usize,
    /// `node icon`.
    pub node_icon: usize,
    /// `node has_note`.
    pub node_has_note: usize,
    /// `edge id`.
    pub edge_id: usize,
    /// `edge source`.
    pub edge_source: usize,
    /// `edge target`.
    pub edge_target: usize,
    /// `edge kind`.
    pub edge_kind: usize,
    /// `edge label`.
    pub edge_label: usize,
    /// `edge record_id`.
    pub record_id: usize,
    /// `edge directed`.
    pub directed: usize,
    /// `edge child_first`.
    pub child_first: usize,
}

/// Bytes, plus where each section is in them.
pub(super) struct Encoded {
    /// The document.
    pub bytes: Vec<u8>,
    /// Section starts.
    pub marks: Marks,
}

impl Encoded {
    /// Overwrite one `u32` cell of the column at `column` in `row`.
    pub fn patch_u32(&mut self, column: usize, row: u32, value: u32) {
        self.write(column + 4 * row as usize, &value.to_le_bytes());
    }

    /// Overwrite one `f64` cell of the column at `column` in `row`.
    pub fn patch_f64(&mut self, column: usize, row: u32, value: f64) {
        self.write(column + 8 * row as usize, &value.to_bits().to_le_bytes());
    }

    /// Overwrite header word `word` (0..8).
    pub fn patch_header(&mut self, word: usize, value: u32) {
        self.write(4 * word, &value.to_le_bytes());
    }

    fn write(&mut self, at: usize, bytes: &[u8]) {
        self.bytes[at..at + bytes.len()].copy_from_slice(bytes);
    }
}

/// A document under construction: strings interned by content, rows in written order.
#[derive(Default)]
pub(super) struct Doc {
    strings: Vec<String>,
    index: BTreeMap<String, u32>,
    nodes: Vec<NodeCells>,
    edges: Vec<EdgeCells>,
}

/// The blob, its offsets and where every column lands. Computed before a byte is written, so
/// the size the decoder later demands is the size that is produced.
struct Plan {
    blob: String,
    offsets: Vec<u32>,
    pad: usize,
    marks: Marks,
    total: usize,
}

impl Doc {
    /// An empty document.
    pub(super) fn new() -> Self {
        Self::default()
    }

    /// Interns `value` and returns its index. Duplicate content gets the same index, which
    /// is legal: two table entries may hold the same bytes, and the arena collapses them.
    pub(super) fn s(&mut self, value: &str) -> u32 {
        if let Some(&at) = self.index.get(value) {
            return at;
        }
        let at = u32::try_from(self.strings.len()).expect("a table below u32::MAX");
        self.strings.push(value.to_owned());
        self.index.insert(value.to_owned(), at);
        at
    }

    /// Appends `value` to the table **without** consulting the dedupe map, so two entries
    /// hold the same bytes. Legal, and what an encoder that does not dedupe produces.
    pub(super) fn duplicate(&mut self, value: &str) -> u32 {
        let at = self.string_count();
        self.strings.push(value.to_owned());
        at
    }

    /// Appends a node row and hands it back to be filled in.
    pub(super) fn node(&mut self) -> &mut NodeCells {
        self.nodes.push(NodeCells {
            weight: 0.0,
            version: 0.0,
            id: ABSENT,
            kind: ABSENT,
            database: ABSENT,
            source: ABSENT,
            label: ABSENT,
            group: ABSENT,
            icon: ABSENT,
            has_note: 0,
        });
        self.nodes.last_mut().expect("just pushed")
    }

    /// Appends an edge row and hands it back to be filled in.
    pub(super) fn edge(&mut self) -> &mut EdgeCells {
        self.edges.push(EdgeCells {
            strength: 0.0,
            id: ABSENT,
            kind: ABSENT,
            label: ABSENT,
            record_id: ABSENT,
            source: ABSENT,
            target: ABSENT,
            directed: 0,
            child_first: 0,
        });
        self.edges.last_mut().expect("just pushed")
    }

    /// The blob's length in bytes, for a refusal test that patches the closing offset.
    pub(super) fn blob_len(&self) -> u32 {
        self.strings.iter().map(|value| value.len() as u32).sum()
    }

    /// How many entries the string table holds.
    pub(super) fn string_count(&self) -> u32 {
        u32::try_from(self.strings.len()).expect("a table below u32::MAX")
    }

    /// Every section's byte offset, from the counts alone.
    fn plan(&self) -> Plan {
        let mut offsets = vec![0u32];
        for value in &self.strings {
            let next = offsets[offsets.len() - 1]
                + u32::try_from(value.len()).expect("a string below u32::MAX");
            offsets.push(next);
        }
        let blob: String = self.strings.concat();
        let (nodes, edges) = (self.nodes.len(), self.edges.len());
        let offsets_at = 32;
        let blob_at = offsets_at + 4 * offsets.len();
        // The pad is measured from the end of the blob: the *columns* must start on an
        // eight-byte boundary from the start of the buffer, not the blob.
        let pad = (8 - (blob_at + blob.len()) % 8) % 8;
        let mut at = blob_at + blob.len() + pad;
        let mut step = |n: usize| {
            let here = at;
            at += n;
            here
        };
        let node4 = 4 * nodes;
        let edge4 = 4 * edges;
        let marks = Marks {
            offsets: offsets_at,
            blob: blob_at,
            pad: blob_at + blob.len(),
            weight: step(8 * nodes),
            version: step(8 * nodes),
            strength: step(8 * edges),
            node_id: step(node4),
            node_kind: step(node4),
            node_database: step(node4),
            node_source: step(node4),
            node_label: step(node4),
            node_group: step(node4),
            node_icon: step(node4),
            node_has_note: step(node4),
            edge_id: step(edge4),
            edge_source: step(edge4),
            edge_target: step(edge4),
            edge_kind: step(edge4),
            edge_label: step(edge4),
            record_id: step(edge4),
            directed: step(edge4),
            child_first: step(edge4),
        };
        Plan {
            blob,
            offsets,
            pad,
            marks,
            total: at,
        }
    }

    /// The bytes, and where each section is.
    pub(super) fn encode(&self) -> Encoded {
        let plan = self.plan();
        let mut out = vec![0u8; plan.total];
        let (nodes, edges) = (self.nodes.len(), self.edges.len());
        for (word, value) in [
            layout::MAGIC,
            layout::VERSION,
            nodes as u32,
            edges as u32,
            self.strings.len() as u32,
            plan.blob.len() as u32,
            0,
            0,
        ]
        .into_iter()
        .enumerate()
        {
            out[4 * word..4 * word + 4].copy_from_slice(&value.to_le_bytes());
        }
        for (i, offset) in plan.offsets.iter().enumerate() {
            let at = plan.marks.offsets + 4 * i;
            out[at..at + 4].copy_from_slice(&offset.to_le_bytes());
        }
        out[plan.marks.blob..plan.marks.blob + plan.blob.len()]
            .copy_from_slice(plan.blob.as_bytes());
        debug_assert!(
            plan.pad < 8
                && out[plan.marks.blob + plan.blob.len()..plan.total]
                    .iter()
                    .all(|b| *b == 0)
        );
        // One column at a time, in contract order: the buffer is structure of arrays, so a
        // row's fields are not neighbours and a per-row write would straddle two columns.
        f64_column(
            &mut out,
            plan.marks.weight,
            self.nodes.iter().map(|n| n.weight),
        );
        f64_column(
            &mut out,
            plan.marks.version,
            self.nodes.iter().map(|n| n.version),
        );
        f64_column(
            &mut out,
            plan.marks.strength,
            self.edges.iter().map(|e| e.strength),
        );
        for (i, node) in self.nodes.iter().enumerate() {
            let cells = [
                node.id,
                node.kind,
                node.database,
                node.source,
                node.label,
                node.group,
                node.icon,
                node.has_note,
            ];
            u32_column(&mut out, plan.marks.node_id, 4 * nodes, &cells, i);
        }
        for (i, edge) in self.edges.iter().enumerate() {
            let cells = [
                edge.id,
                edge.source,
                edge.target,
                edge.kind,
                edge.label,
                edge.record_id,
                edge.directed,
                edge.child_first,
            ];
            u32_column(&mut out, plan.marks.edge_id, 4 * edges, &cells, i);
        }
        Encoded {
            bytes: out,
            marks: plan.marks,
        }
    }
}

/// One `f64` column of `values` at `at`.
fn f64_column(out: &mut [u8], at: usize, values: impl Iterator<Item = f64>) {
    for (row, value) in values.enumerate() {
        let here = at + 8 * row;
        out[here..here + 8].copy_from_slice(&value.to_bits().to_le_bytes());
    }
}

/// `count` consecutive `u32` columns starting at `first`, one cell per row. `stride` is
/// one column's length in bytes: the columns are neighbours, not interleaved per row.
fn u32_column(out: &mut [u8], first: usize, stride: usize, values: &[u32], row: usize) {
    for (i, value) in values.iter().enumerate() {
        let at = first + stride * i + 4 * row;
        out[at..at + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// A document with everything the contract can carry on one edge: both nodes fully
/// populated, one node with every optional absent, one `child_first` edge, a `-0.0` weight
/// and a subnormal version. Every happy-path and refusal test starts from it so a change to
/// the encoder moves all of them together.
pub(super) fn full() -> Doc {
    let mut doc = Doc::new();
    let (n0, n1, db, src, lab, grp, icon, rec, note, rel) = (
        doc.s("n-0"),
        doc.s("n-1"),
        doc.s("db-0"),
        doc.s("studio"),
        doc.s("Graph notes"),
        doc.s("Epsilon"),
        doc.s("\u{1f33f}"),
        doc.s("record"),
        doc.s("note"),
        doc.s("relation"),
    );
    let e0 = doc.s("e-0");
    let label = doc.s("relation");
    let record = doc.s("rec-7");
    let n = doc.node();
    *n = NodeCells {
        weight: -0.0,
        version: f64::from_bits(1),
        id: n0,
        kind: rec,
        database: db,
        source: src,
        label: lab,
        group: grp,
        icon,
        has_note: 1,
    };
    let n = doc.node();
    *n = NodeCells {
        weight: 1.0,
        version: 2.0,
        id: n1,
        kind: note,
        database: ABSENT,
        source: src,
        label: lab,
        group: ABSENT,
        icon: ABSENT,
        has_note: 0,
    };
    let e = doc.edge();
    *e = EdgeCells {
        strength: 0.5,
        id: e0,
        source: 0,
        target: 1,
        kind: rel,
        label,
        record_id: record,
        directed: 1,
        child_first: 1,
    };
    doc
}

#[cfg(test)]
mod refusals;

#[cfg(test)]
mod shape;
