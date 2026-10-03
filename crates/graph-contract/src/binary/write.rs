//! Writing the binary face: the output length is counted from the parts before a byte
//! is written, so the buffer is allocated once at its final size instead of doubling
//! its way there. The count and the write are two functions over the same parts, so a
//! column added to one and not the other is caught by the assertion at the end of
//! [`bytes`], not by a snapshot that is a megabyte short.

use super::{Snapshot, SnapshotParts, StringTable, padding};
use crate::geometry::{EdgeGeometry, Paths};
use crate::notes::carries_notes;
use crate::snapshot::HEADER_LEN;

/// `snapshot`'s bytes, allocated once at the length [`byte_len`] counted.
///
/// The [`debug_assert`] at the end is the contract between the two halves of this file:
/// `byte_len` is an independent count of what the `put_` calls below write, so a column
/// added to one and forgotten in the other fails there.
pub(super) fn bytes(snapshot: &Snapshot) -> Vec<u8> {
    let parts = snapshot.parts();
    let len = byte_len(parts);
    let mut out = Vec::with_capacity(len);
    snapshot.header().encode(&mut out);
    put_table(&mut out, &parts.node_ids);
    put_table(&mut out, &parts.edge_ids);
    put_u32s(&mut out, &parts.source);
    put_u32s(&mut out, &parts.target);
    for (_, column) in parts.nodes.columns_dim(parts.z.as_deref()) {
        put_f32s(&mut out, column);
    }
    if let Some(paths) = paths(&parts.edges) {
        if let EdgeGeometry::Curve { degree, .. } = parts.edges {
            put_u32s(&mut out, &[degree]);
        }
        put_paths(&mut out, paths);
    }
    if carries_notes(parts.version) {
        put_u32s(&mut out, &[parts.notes.len()]);
        put_u32s(&mut out, &parts.notes.code);
        put_u32s(&mut out, &parts.notes.index);
    }
    debug_assert_eq!(out.len(), len, "byte_len counted what the writer wrote");
    out
}

/// `parts`' exact output length: the header, then every section the layout writes, in the
/// order [`bytes`] writes them. Counted in `u64` because a `usize` count of a 32-bit
/// target's own columns can overflow it; a snapshot big enough to do that cannot be
/// allocated anyway, and `Vec::with_capacity` says so.
pub(super) fn byte_len(parts: &SnapshotParts) -> usize {
    let mut len = u64::from(HEADER_LEN);
    for table in [&parts.node_ids, &parts.edge_ids] {
        len += 4 * table.offsets().len() as u64;
        let text = table.bytes().len();
        len += (text + padding(text)) as u64;
    }
    len += 4 * (parts.source.len() + parts.target.len()) as u64;
    for (_, column) in parts.nodes.columns_dim(parts.z.as_deref()) {
        len += 4 * column.len() as u64;
    }
    if let Some(paths) = paths(&parts.edges) {
        len += 4 * (paths.offsets.len() + paths.pts.len()) as u64;
    }
    if let EdgeGeometry::Curve { .. } = parts.edges {
        len += 4;
    }
    if carries_notes(parts.version) {
        len += 4 * (1 + parts.notes.code.len() + parts.notes.index.len()) as u64;
    }
    usize::try_from(len).unwrap_or(usize::MAX)
}

/// The path columns, for every edge kind that has them; `None` for a straight line.
fn paths(edges: &EdgeGeometry) -> Option<&Paths> {
    match edges {
        EdgeGeometry::Line => None,
        EdgeGeometry::Polyline(paths) | EdgeGeometry::Curve { paths, .. } => Some(paths),
    }
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
