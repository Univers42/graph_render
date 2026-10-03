//! Reading the binary face: every column is taken only after its length is checked
//! against what is left, so a header that claims more than the payload holds is refused
//! before anything is allocated for it.

use super::{Snapshot, SnapshotParts, StringTable, padding};
use crate::geometry::{
    EdgeGeometry, EdgeGeometryKind, NodeGeometry, NodeGeometryKind, Paths, index_u32,
};
use crate::notes::{Notes, carries_notes};
use crate::snapshot::{Dim, HEADER_LEN, SnapshotError, SnapshotHeader};

pub(super) fn decode(bytes: &[u8]) -> Result<Snapshot, SnapshotError> {
    let header = SnapshotHeader::decode(bytes).map_err(SnapshotError::Header)?;
    let (n, m) = (header.node_count, header.edge_count);
    let mut r = Reader(&bytes[HEADER_LEN as usize..]);
    let node_ids = r.table("node.id", n)?;
    let edge_ids = r.table("edge.id", m)?;
    let source = r.u32s("edge.source", u64::from(m))?;
    let target = r.u32s("edge.target", u64::from(m))?;
    let (nodes, z) = r.nodes(header.node_kind, header.dim, n)?;
    let edges = r.edges(header.edge_kind, m)?;
    let notes = if carries_notes(header.version) {
        r.notes()?
    } else {
        Notes::default()
    };
    if !r.0.is_empty() {
        let count = u64::try_from(r.0.len()).unwrap_or(u64::MAX);
        return Err(SnapshotError::TrailingBytes { count });
    }
    Snapshot::new(SnapshotParts {
        version: header.version,
        node_ids,
        edge_ids,
        source,
        target,
        nodes,
        z,
        edges,
        notes,
    })
}

/// The bytes not read yet.
struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take(&mut self, column: &'static str, len: u64) -> Result<&'a [u8], SnapshotError> {
        let len = match usize::try_from(len) {
            Ok(len) if len <= self.0.len() => len,
            _ => return Err(SnapshotError::Truncated { column }),
        };
        let (head, rest) = self.0.split_at(len);
        self.0 = rest;
        Ok(head)
    }

    fn u32s(&mut self, column: &'static str, count: u64) -> Result<Vec<u32>, SnapshotError> {
        let bytes = self.take(column, count.saturating_mul(4))?;
        let (words, _) = bytes.as_chunks::<4>();
        Ok(words.iter().map(|w| u32::from_le_bytes(*w)).collect())
    }

    fn f32s(&mut self, column: &'static str, count: u64) -> Result<Vec<f32>, SnapshotError> {
        let words = self.u32s(column, count)?;
        Ok(words.into_iter().map(f32::from_bits).collect())
    }

    /// The ids of `count` entries, as a CSR-shaped string table: `count + 1` offsets,
    /// then the bytes they name, then padding to a word.
    ///
    /// Ponytail: the `String` is reserved with `String::with_capacity(len)` where `len` is
    /// the last offset, so the allocation grows with the caller's declared id table and
    /// has no ceiling of its own — `Reader::take` bounds it 1:1 by the bytes left in the
    /// payload, which is a bound on what one snapshot can claim, not on what a caller may
    /// claim. The escape hatch is whatever ceiling the host applies to the input before it
    /// is decoded.
    fn table(&mut self, column: &'static str, count: u32) -> Result<StringTable, SnapshotError> {
        let offsets = self.u32s(column, u64::from(count) + 1)?;
        check_offsets(column, &offsets)?;
        let len = offsets[offsets.len() - 1] as usize;
        let bytes = self.take(column, len as u64)?;
        if self
            .take(column, padding(len) as u64)?
            .iter()
            .any(|&b| b != 0)
        {
            return Err(SnapshotError::Padding { column });
        }
        let mut text = String::with_capacity(len);
        for (i, w) in offsets.windows(2).enumerate() {
            let id = core::str::from_utf8(&bytes[w[0] as usize..w[1] as usize]);
            let index = index_u32(i);
            text.push_str(id.map_err(|_| SnapshotError::Utf8 { column, index })?);
        }
        Ok(StringTable { offsets, text })
    }

    /// The node columns, in the order `dim` says: a 3D snapshot's z is taken right after
    /// `y`, so the sizes it pushes along are read from the shifted position, never a
    /// fixed offset.
    fn nodes(
        &mut self,
        kind: NodeGeometryKind,
        dim: Dim,
        n: u32,
    ) -> Result<(NodeGeometry, Option<Vec<f32>>), SnapshotError> {
        let n = u64::from(n);
        let (x, y) = (self.f32s("node.x", n)?, self.f32s("node.y", n)?);
        let z = dim.is_3d().then(|| self.f32s("node.z", n)).transpose()?;
        let nodes = match kind {
            NodeGeometryKind::Point => NodeGeometry::Point { x, y },
            NodeGeometryKind::Circle => NodeGeometry::Circle {
                x,
                y,
                r: self.f32s("node.r", n)?,
            },
            NodeGeometryKind::Box => NodeGeometry::Box {
                x,
                y,
                w: self.f32s("node.w", n)?,
                h: self.f32s("node.h", n)?,
            },
        };
        Ok((nodes, z))
    }

    fn edges(&mut self, kind: EdgeGeometryKind, m: u32) -> Result<EdgeGeometry, SnapshotError> {
        Ok(match kind {
            EdgeGeometryKind::Line => EdgeGeometry::Line,
            EdgeGeometryKind::Polyline => EdgeGeometry::Polyline(self.paths(m)?),
            EdgeGeometryKind::Curve => {
                let degree = self.u32s("edge.degree", 1)?[0];
                let paths = self.paths(m)?;
                EdgeGeometry::Curve { degree, paths }
            }
        })
    }

    /// `k`, then `k` codes, then `k` indices; whether they are a valid, canonical set is
    /// [`Snapshot::new`]'s to say.
    fn notes(&mut self) -> Result<Notes, SnapshotError> {
        let k = u64::from(self.u32s("note.count", 1)?[0]);
        let code = self.u32s("note.code", k)?;
        let index = self.u32s("note.index", k)?;
        Ok(Notes { code, index })
    }

    /// The offsets, then as many coordinates as the last offset calls for. The offsets are
    /// checked first, so a vector that has already broken a rule never sizes the read; how
    /// many coordinates it does name is still [`Paths::check`]'s to say, through
    /// [`Snapshot::new`].
    fn paths(&mut self, m: u32) -> Result<Paths, SnapshotError> {
        let offsets = self.u32s("edge.offsets", u64::from(m) + 1)?;
        check_offsets("edge.offsets", &offsets)?;
        let points = u64::from(offsets[offsets.len() - 1]);
        let pts = self.f32s("edge.pts", 2 * points)?;
        Ok(Paths { offsets, pts })
    }
}

/// What every CSR offsets column owes its data before that data may be sized from it:
/// from 0, and never decreasing. Both string tables and point tables are read this way,
/// so a malformed offsets vector is refused under its own column rather than as a short
/// payload somewhere downstream of it.
fn check_offsets(column: &'static str, offsets: &[u32]) -> Result<(), SnapshotError> {
    let bad = |index: usize| SnapshotError::Offsets {
        column,
        index: index_u32(index),
    };
    if offsets[0] != 0 {
        return Err(bad(0));
    }
    if let Some(i) = offsets.windows(2).position(|w| w[1] < w[0]) {
        return Err(bad(i + 1));
    }
    Ok(())
}
