//! The binary face of a snapshot: the bytes the hash is taken over, laid out exactly as
//! `docs/contract/binary-layout.md` says — that document is authoritative, this module
//! follows it. Columnar and little-endian; every section is a whole number of 4-byte
//! words, so every column starts 4-byte aligned and a reader can view it in place.
//!
//! A [`Snapshot`] can only be built valid ([`Snapshot::new`]), so writing one cannot
//! fail and the decoder refuses exactly what the constructor refuses.

use crate::geometry::{EdgeGeometry, NodeGeometry, check_len, index_u32};
use crate::notes::Notes;
use crate::snapshot::{Dim, ReadError, SnapshotError, SnapshotHeader, StageCount, carries_dim};
use crate::version::{FormatVersion, check_readable};
use std::collections::HashSet;
use std::hash::{BuildHasherDefault, DefaultHasher};

mod decode;
mod write;

/// Bytes to allow per id when reserving the text of a table whose length is not known
/// before it is read.
const ID_BYTES: usize = 16;

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
    ///
    /// Both buffers are reserved from the iterator's item count before anything is
    /// written, so a million ids cost one allocation each instead of a doubling every
    /// time one of them fills. `offsets` is then exact — `count + 1` words for `count`
    /// items — and nothing collected: the iterator is read once, through.
    ///
    /// Caveat: `text` cannot be sized exactly without reading the ids twice, so
    /// `ID_BYTES` is a guess at an id's length. Ids longer than it cost one
    /// reallocation; ids shorter than it leave up to `ID_BYTES` bytes per id reserved
    /// and never filled, 16 MB at a million six-byte ids. It sizes the first allocation
    /// only: `text` still grows to whatever the ids really are, and `try_reserve` reports
    /// a host that cannot give it even this much as [`SnapshotError::Capacity`], the same
    /// refusal the `u32` overflow below gives.
    pub fn from_strs<'a>(
        column: &'static str,
        items: impl IntoIterator<Item = &'a str>,
    ) -> Result<Self, SnapshotError> {
        let items = items.into_iter();
        let (count, _) = items.size_hint();
        let mut offsets = Vec::new();
        let mut text = String::new();
        offsets
            .try_reserve_exact(count.saturating_add(1))
            .map_err(|_| SnapshotError::Capacity { column })?;
        text.try_reserve(count.saturating_mul(ID_BYTES))
            .map_err(|_| SnapshotError::Capacity { column })?;
        offsets.push(0);
        for item in items {
            text.push_str(item);
            let end = u32::try_from(text.len());
            match end {
                Ok(end) if offsets.len() <= u32::MAX as usize => offsets.push(end),
                _ => return Err(SnapshotError::Capacity { column }),
            }
        }
        Ok(Self { offsets, text })
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
    ///
    /// D4: the set answers membership only and is **never iterated** — the answer is a
    /// position in this table's own order, read from `iter()`, so the set's internal
    /// order cannot reach it. The hasher is `DefaultHasher`'s under `BuildHasherDefault`,
    /// so it is fixed across runs and hosts (no seed, no randomness).
    ///
    /// Caveat: fixed keys are public, so a decoded payload built to collide can push this
    /// from O(n) toward O(n²) time; the answer stays exact. Escape: a `BTreeSet` here,
    /// 4x slower at a million ids (`docs/measurements/perf-snapshot-build.md`).
    fn first_repeat(&self) -> Option<u32> {
        let mut seen: HashSet<&str, BuildHasherDefault<DefaultHasher>> =
            HashSet::with_capacity_and_hasher(self.len() as usize, Default::default());
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
    /// Node geometry, one discriminant for all nodes. The `x` and `y` columns, and the
    /// sizes; the third coordinate, if any, is [`z`](Self::z), not a field here, so that
    /// a 2D snapshot's geometry type is unchanged by 3D existing.
    pub nodes: NodeGeometry,
    /// The z column, `Some` for a 3D snapshot and `None` for a 2D one — which is the only
    /// way to tell them apart, and is what the header's `dim` is derived from. So `Some`
    /// here and `dim = 1` in the header can never disagree.
    pub z: Option<Vec<f32>>,
    /// Edge geometry, one discriminant for all edges.
    pub edges: EdgeGeometry,
    /// What the stages repaired or approximated (`crate::notes`); none below 0.3.
    pub notes: Notes,
}

impl SnapshotParts {
    /// The header's `dim`: the z column's presence, and nothing else. A snapshot has one
    /// dimension for the whole payload, so this is the only place it is decided.
    pub fn dim(&self) -> Dim {
        match self.z {
            Some(_) => Dim::D3,
            None => Dim::D2,
        }
    }
}

/// A snapshot every reader of this version accepts: the only kind that can exist.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot(SnapshotParts);

impl Snapshot {
    /// Checks `parts` against every rule of the layout: a readable version, unique ids,
    /// endpoints inside the node table, geometry that fits the counts, finite (D9), and
    /// notes from the closed set in canonical order, only where the version carries them.
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
        check_dimmed(parts.dim(), parts.version)?;
        parts.nodes.check(n, parts.z.as_deref())?;
        parts.edges.check(m)?;
        parts.notes.check(parts.version, m)?;
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
            dim: self.0.dim(),
            stage_count: StageCount::ONE,
            node_count: self.0.node_ids.len(),
            edge_count: self.0.edge_ids.len(),
        }
    }

    /// The binary face.
    pub fn to_bytes(&self) -> Vec<u8> {
        write::bytes(self)
    }

    /// Reads the binary face, refusing anything [`Snapshot::new`] refuses, a newer
    /// major, a truncated column and trailing bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SnapshotError> {
        decode::decode(bytes)
    }
}

/// A z column needs a version that carries `dim`: a 0.4 label is required of a 3D
/// snapshot, or the bytes would claim a dimension the version cannot name. A 2D snapshot
/// may be labelled 0.4 — [`label_for`](crate::snapshot::label_for) will not choose that,
/// but a reader of one is no worse off. Refusing here rather than writing an
/// unnameable dimension is the whole point of the label rule.
fn check_dimmed(dim: Dim, version: FormatVersion) -> Result<(), SnapshotError> {
    if dim.is_3d() && !carries_dim(version) {
        return Err(SnapshotError::DimUnnameable { version });
    }
    Ok(())
}

/// Zero bytes that bring `len` up to a multiple of 4.
pub(crate) const fn padding(len: usize) -> usize {
    (4 - len % 4) % 4
}

#[cfg(test)]
mod tests;
