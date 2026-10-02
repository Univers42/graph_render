//! Column id resolution (`docs/contract/wasm-abi.md` "Columns", C3): a target-independent
//! mapping from a run's [`Snapshot`] geometry to a borrowed slice. The pointer layer
//! (`exports.rs`) turns what [`column`] returns into a `(ptr, len)` pair; nothing here
//! touches a memory address, so every column — including the ones no registered layout
//! emits yet, Circle/Box nodes and Polyline/Curve edges — is unit-tested natively by
//! building the [`Snapshot`] by hand.
//!
//! Ids are append-only. `id::NOTE_CODE`/`id::NOTE_INDEX` are reserved for Phase 3's
//! `notes` section (contract 0.3) — the two fields it brings are `note.code` and
//! `note.index`, which is why these two ids are numbered before the edge-path ids: that
//! merge fills these slots rather than renumbering anything shipped here. This snapshot
//! type has no such field yet, so both are [`Column::Absent`] unconditionally, for every
//! graph, until that merge.
//!
//! `id::NODE_Z` is the 3D z column (contract 0.4), appended after every shipped id for the
//! same reason. This layer is transport: it carries a 3D snapshot rather than refusing it,
//! and [`dim`] is how a consumer learns a snapshot is one instead of parsing byte 14 itself.

use crate::errors::Code;
use graph_contract::binary::Snapshot;
use graph_contract::geometry::EdgeGeometry;

/// A column's borrowed data, or [`Column::Absent`] for a ptr/len of `(0, 0)` — reserved,
/// or not this snapshot's geometry kind. Absent and "present but length 0" read the same
/// `(0, 0)` on the wire; the caller tells them apart via `gm_geometry_kind`/
/// `gm_edge_geometry_kind`, which is why this module exposes those too.
pub enum Column<'a> {
    /// Reserved, or this snapshot's node/edge kind does not carry this column.
    Absent,
    /// `f32` elements, e.g. `x`, `y`, `r`, edge `pts`.
    F32(&'a [f32]),
    /// `u32` elements, e.g. `edge.source`, `edge.offsets`.
    U32(&'a [u32]),
}

/// Column ids, append-only (C3). Never renumber a shipped id.
pub mod id {
    /// Node `x`, `f32 x n`. Every node kind.
    pub const NODE_X: u32 = 0;
    /// Node `y`, `f32 x n`. Every node kind.
    pub const NODE_Y: u32 = 1;
    /// Node radius, `f32 x n`. Circle only.
    pub const NODE_R: u32 = 2;
    /// Node width, `f32 x n`. Box only.
    pub const NODE_W: u32 = 3;
    /// Node height, `f32 x n`. Box only.
    pub const NODE_H: u32 = 4;
    /// Edge source, dense node index, `u32 x m`. Every edge kind.
    pub const EDGE_SOURCE: u32 = 5;
    /// Edge target, dense node index, `u32 x m`. Every edge kind.
    pub const EDGE_TARGET: u32 = 6;
    /// Reserved: Phase 3's `note.code`, `u32 x k`. Always absent until that merge.
    pub const NOTE_CODE: u32 = 7;
    /// Reserved: Phase 3's `note.index`, `u32 x k`. Always absent until that merge.
    pub const NOTE_INDEX: u32 = 8;
    /// Edge path offsets, `u32 x (m + 1)`. Polyline and Curve only.
    pub const EDGE_OFFSETS: u32 = 9;
    /// Edge path points, `f32 x 2*offsets[m]`. Polyline and Curve only.
    pub const EDGE_PTS: u32 = 10;
    /// The one curve degree for the whole snapshot, `u32 x 1`. Curve only.
    pub const EDGE_CURVE_DEGREE: u32 = 11;
    /// Node `z`, `f32 x n`. 3D only (`dim = 1` in the header); absent for every 2D
    /// snapshot, whatever its node kind. Appended last, never renumbering a shipped id.
    pub const NODE_Z: u32 = 12;
}

/// The column `column_id` names in `snapshot`, or [`Column::Absent`].
pub fn column(snapshot: &Snapshot, column_id: u32) -> Column<'_> {
    let parts = snapshot.parts();
    match column_id {
        id::NODE_X | id::NODE_Y | id::NODE_R | id::NODE_W | id::NODE_H | id::NODE_Z => {
            node_column(parts, column_id)
        }
        id::EDGE_SOURCE => Column::U32(&parts.source),
        id::EDGE_TARGET => Column::U32(&parts.target),
        id::EDGE_OFFSETS | id::EDGE_PTS | id::EDGE_CURVE_DEGREE => {
            edge_column(&parts.edges, column_id)
        }
        // Reserved for Phase 3's `note.code`/`note.index` (contract 0.3): named here, not
        // folded into the catch-all below, so the reservation is a real arm a reader —
        // and the dead-code lint — can see, not just a doc comment.
        id::NOTE_CODE | id::NOTE_INDEX => Column::Absent,
        _ => Column::Absent,
    }
}

/// `columns_dim` with the snapshot's own z, so a `NODE_Z` request finds the z column where
/// it was written — after `y` — instead of falling through to some other column's name.
fn node_column(parts: &graph_contract::binary::SnapshotParts, column_id: u32) -> Column<'_> {
    let wire_name = match column_id {
        id::NODE_X => "x",
        id::NODE_Y => "y",
        id::NODE_R => "r",
        id::NODE_W => "w",
        id::NODE_Z => "z",
        _ => "h",
    };
    parts
        .nodes
        .columns_dim(parts.z.as_deref())
        .into_iter()
        .find(|(name, _)| *name == wire_name)
        .map_or(Column::Absent, |(_, values)| Column::F32(values))
}

fn edge_column(edges: &EdgeGeometry, column_id: u32) -> Column<'_> {
    match (edges, column_id) {
        (EdgeGeometry::Polyline(paths), id::EDGE_OFFSETS) => Column::U32(&paths.offsets),
        (EdgeGeometry::Polyline(paths), id::EDGE_PTS) => Column::F32(&paths.pts),
        (EdgeGeometry::Curve { paths, .. }, id::EDGE_OFFSETS) => Column::U32(&paths.offsets),
        (EdgeGeometry::Curve { paths, .. }, id::EDGE_PTS) => Column::F32(&paths.pts),
        (EdgeGeometry::Curve { degree, .. }, id::EDGE_CURVE_DEGREE) => {
            Column::U32(std::slice::from_ref(degree))
        }
        _ => Column::Absent,
    }
}

/// The node geometry tag (`docs/contract/binary-layout.md`): `0` Point, `1` Circle, `2`
/// Box. `gm_geometry_kind`'s body.
pub fn node_kind_tag(snapshot: &Snapshot) -> u8 {
    snapshot.header().node_kind.tag()
}

/// The edge geometry tag: `0` Line, `1` Polyline, `2` Curve. `gm_edge_geometry_kind`'s
/// body — an extra export beyond the phase's minimum surface (C3: "the edge kind must be
/// readable through the ABI"), needed because `gm_geometry_kind` alone only names nodes.
pub fn edge_kind_tag(snapshot: &Snapshot) -> u8 {
    snapshot.header().edge_kind.tag()
}

/// Column `column_id` of `snapshot` as the wire reads it: its address, or with
/// `want_len` its element count. `gm_column_ptr`/`gm_column_len`'s body.
pub fn column_wire(snapshot: &Snapshot, column_id: u32, want_len: bool) -> Result<u32, Code> {
    let (ptr, len) = match column(snapshot, column_id) {
        Column::Absent => return Ok(0),
        Column::F32(v) => (v.as_ptr() as usize, v.len()),
        Column::U32(v) => (v.as_ptr() as usize, v.len()),
    };
    // C3: absent and present-but-empty both read (0, 0); an empty `Vec`'s pointer is a
    // dangling, nonzero address that names no memory.
    if len == 0 {
        return Ok(0);
    }
    crate::wire::to_wire(if want_len { len } else { ptr })
}

/// How many dimensions `snapshot` carries: `0` 2D, `1` 3D. `gm_dim`'s body. The wasm
/// layer is transport and does not refuse 3D — it carries it, and says so here, so a
/// consumer is not left parsing byte 14 of a raw snapshot itself.
pub fn dim(snapshot: &Snapshot) -> u8 {
    snapshot.header().dim.get()
}

/// Whether any node or edge coordinate in `snapshot` is NaN or infinite. Checked again
/// here — not trusted from construction — because the SDK's typed-array views are
/// writable aliases directly into these `Vec`s (D9, C8): `Snapshot::to_bytes`/`to_json`
/// do not re-check, so a tampered view would otherwise reach the wire unnoticed.
pub fn has_non_finite(snapshot: &Snapshot) -> bool {
    let parts = snapshot.parts();
    // `columns_dim` with the snapshot's own z: a tampered z must be caught here too, or a
    // 3D snapshot could reach the wire carrying a NaN depth nobody looked at.
    let nodes_bad = parts
        .nodes
        .columns_dim(parts.z.as_deref())
        .into_iter()
        .any(|(_, values)| values.iter().any(|v| !v.is_finite()));
    let edges_bad = match &parts.edges {
        EdgeGeometry::Line => false,
        EdgeGeometry::Polyline(paths) => paths.pts.iter().any(|v| !v.is_finite()),
        EdgeGeometry::Curve { paths, .. } => paths.pts.iter().any(|v| !v.is_finite()),
    };
    nodes_bad || edges_bad
}

#[cfg(test)]
mod tests;
