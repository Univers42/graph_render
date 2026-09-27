//! The binary face of a snapshot: the bytes the hash is taken over, laid out exactly as
//! `docs/contract/binary-layout.md` says — that document is authoritative, this module
//! follows it. Columnar and little-endian; every section is a whole number of 4-byte
//! words, so every column starts 4-byte aligned and a reader can view it in place.
//!
//! A [`Snapshot`] can only be built valid ([`Snapshot::new`]), so writing one cannot
//! fail and the decoder refuses exactly what the constructor refuses.

use crate::geometry::{EdgeGeometry, NodeGeometry, Paths, check_len, index_u32};
use crate::snapshot::{ReadError, SnapshotError, SnapshotHeader, StageCount};
use crate::version::{FormatVersion, check_readable};
use std::collections::BTreeSet;

mod decode;

/// Strings stored CSR-shaped: string `i` is `bytes[offsets[i]..offsets[i + 1]]`. The
/// same shape as the adjacency and the edge paths, so one mental model covers them all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTable {
    offsets: Vec<u32>,
    text: String,
}

impl Default for StringTable {
    fn default() -> Self {
        Self {
            offsets: vec![0],
            text: String::new(),
        }
    }
}

impl StringTable {
    /// A table of `items` in order, refused when more than `u32` counts.
    pub fn from_strs<'a>(
        column: &'static str,
        items: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, SnapshotError> {
        let mut table = Self::default();
        for item in items {
            table.text.push_str(item);
            let end = u32::try_from(table.text.len());
            match end {
                Ok(end) if table.offsets.len() <= u32::MAX as usize => table.offsets.push(end),
                _ => return Err(SnapshotError::Capacity { column }),
            }
        }
        Ok(table)
    }

    /// Number of strings.
    pub fn len(&self) -> u32 {
        index_u32(self.offsets.len() - 1)
    }

    /// Whether the table holds no string.
    pub fn is_empty(&self) -> bool {
        self.offsets.len() == 1
    }

    /// String `index`, if there is one.
    pub fn get(&self, index: u32) -> Option<&str> {
        let i = index as usize;
        let (start, end) = (*self.offsets.get(i)?, *self.offsets.get(i + 1)?);
        self.text.get(start as usize..end as usize)
    }

    /// Every string, in order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.offsets
            .windows(2)
            .map(|w| &self.text[w[0] as usize..w[1] as usize])
    }

    /// The `len + 1` offsets, as written.
    pub fn offsets(&self) -> &[u32] {
        &self.offsets
    }

    /// Every string's bytes back to back, as written before the padding.
    pub fn bytes(&self) -> &[u8] {
        self.text.as_bytes()
    }

    /// The position of the first string equal to an earlier one.
    fn first_repeat(&self) -> Option<u32> {
        let mut seen = BTreeSet::new();
        self.iter().position(|s| !seen.insert(s)).map(index_u32)
    }
}

/// Everything a snapshot holds, before it is checked.
#[derive(Debug, Clone, PartialEq)]
pub struct SnapshotParts {
    /// The format version it was written in.
    pub version: FormatVersion,
    /// Stable node ids, unique; a node's position here is its position in every column.
    pub node_ids: StringTable,
    /// Stable edge ids, unique.
    pub edge_ids: StringTable,
    /// Each edge's source, as a position in `node_ids`.
    pub source: Vec<u32>,
    /// Each edge's target, as a position in `node_ids`.
    pub target: Vec<u32>,
    /// Node geometry, one discriminant for all nodes.
    pub nodes: NodeGeometry,
    /// Edge geometry, one discriminant for all edges.
    pub edges: EdgeGeometry,
}

/// A snapshot every reader of this version accepts: the only kind that can exist.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot(SnapshotParts);

impl Snapshot {
    /// Checks `parts` against every rule of the layout: a readable version, unique ids,
    /// endpoints inside the node table, and geometry that fits the counts, finite (D9).
    pub fn new(parts: SnapshotParts) -> Result<Self, SnapshotError> {
        check_readable(parts.version)
            .map_err(|newer| SnapshotError::Header(ReadError::UnsupportedMajor(newer)))?;
        let (n, m) = (parts.node_ids.len(), parts.edge_ids.len());
        for (column, table) in [("node.id", &parts.node_ids), ("edge.id", &parts.edge_ids)] {
            if let Some(index) = table.first_repeat() {
                return Err(SnapshotError::DuplicateId { column, index });
            }
        }
        for (column, ends) in [
            ("edge.source", &parts.source),
            ("edge.target", &parts.target),
        ] {
            check_len(column, u64::from(m), ends.len())?;
            if let Some(bad) = ends.iter().position(|&end| end >= n) {
                let index = index_u32(bad);
                return Err(SnapshotError::Endpoint { column, index });
            }
        }
        parts.nodes.check(n)?;
        parts.edges.check(m)?;
        Ok(Self(parts))
    }

    /// What it holds.
    pub fn parts(&self) -> &SnapshotParts {
        &self.0
    }

    /// What it holds, by value.
    pub fn into_parts(self) -> SnapshotParts {
        self.0
    }

    /// The header its bytes start with.
    pub fn header(&self) -> SnapshotHeader {
        SnapshotHeader {
            version: self.0.version,
            node_kind: self.0.nodes.kind(),
            edge_kind: self.0.edges.kind(),
            stage_count: StageCount::ONE,
            node_count: self.0.node_ids.len(),
            edge_count: self.0.edge_ids.len(),
        }
    }

    /// The binary face.
    pub fn to_bytes(&self) -> Vec<u8> {
        let parts = &self.0;
        let mut out = Vec::new();
        self.header().encode(&mut out);
        put_table(&mut out, &parts.node_ids);
        put_table(&mut out, &parts.edge_ids);
        put_u32s(&mut out, &parts.source);
        put_u32s(&mut out, &parts.target);
        for (_, column) in parts.nodes.columns() {
            put_f32s(&mut out, column);
        }
        match &parts.edges {
            EdgeGeometry::Line => {}
            EdgeGeometry::Polyline(paths) => put_paths(&mut out, paths),
            EdgeGeometry::Curve { degree, paths } => {
                put_u32s(&mut out, &[*degree]);
                put_paths(&mut out, paths);
            }
        }
        out
    }

    /// Reads the binary face, refusing anything [`Snapshot::new`] refuses, a newer
    /// major, a truncated column and trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SnapshotError> {
        decode::decode(bytes)
    }
}

/// Zero bytes that bring `len` up to a multiple of 4.
pub(crate) const fn padding(len: usize) -> usize {
    (4 - len % 4) % 4
}

fn put_table(out: &mut Vec<u8>, table: &StringTable) {
    put_u32s(out, table.offsets());
    out.extend_from_slice(table.bytes());
    out.resize(out.len() + padding(table.bytes().len()), 0);
}

fn put_paths(out: &mut Vec<u8>, paths: &Paths) {
    put_u32s(out, &paths.offsets);
    put_f32s(out, &paths.pts);
}

fn put_u32s(out: &mut Vec<u8>, values: &[u32]) {
    values
        .iter()
        .for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
}

fn put_f32s(out: &mut Vec<u8>, values: &[f32]) {
    values
        .iter()
        .for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
}

#[cfg(test)]
mod tests;
