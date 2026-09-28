//! The notes section (format 0.3, user decision D-N): what a stage repaired or
//! approximated, recorded **in** the snapshot — both faces — never only in a log, so a
//! downstream program can tell an exact result from a degraded one.
//!
//! Columnar like every other section: `k: u32`, then `code: u32 × k`, then
//! `index: u32 × k`, the **last** section of the binary face, after edge geometry. The
//! JSON face is `"notes": {"code": [...], "index": [...]}`. A note's `index` is an edge
//! **position** (an index into the edge columns, `edges.id` on the JSON face), or
//! [`SNAPSHOT_WIDE`] — `u32::MAX`, written `4294967295` — for a note about the whole
//! snapshot.
//!
//! **A closed set**, like the geometry tags: codes 1-3 are implemented; 4-6 are allocated
//! to later phases and refused as [`NoteCodeError::Reserved`] (they are not
//! [`NoteCode`] variants, so no writer can emit one); anything else is
//! [`NoteCodeError::Unknown`]. Turning a reserved code on in a later phase is a contract
//! **minor bump**, because a 0.3 reader refuses it.
//!
//! **Canonical**: strictly ascending by `(code, index)`, a total order, so a repeat is
//! refused. A producer sorts before building; [`Snapshot::new`](crate::binary::Snapshot::new) checks the order and
//! never sorts silently. It is the one place the rules below are enforced, so the binary
//! and JSON faces refuse exactly the same notes.
//!
//! Below 0.3 there is no notes section: a 0.2 snapshot reads as `k = 0`, and a snapshot
//! labelled below 0.3 cannot carry a note (see [`carries_notes`]).

use crate::snapshot::SnapshotError;
use crate::version::FormatVersion;
use core::fmt;

/// What a note records. The discriminant is the code on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum NoteCode {
    /// `hierarchy.cycle_edge_dropped`: a hierarchy edge dropped to break a parent cycle;
    /// the index is the edge's position.
    CycleEdgeDropped = 1,
    /// `hierarchy.extra_parent_dropped`: a hierarchy edge dropped because its child
    /// already kept a parent; the index is the edge's position.
    ExtraParentDropped = 2,
    /// `packing.approximate`: circle packing fell back to its approximation, which is
    /// not guaranteed tangent or non-overlapping; the index is [`SNAPSHOT_WIDE`].
    PackingApproximate = 3,
}

/// Codes allocated to later phases and refused as reserved: `4`
/// `dag.dummy_budget_exceeded` and `5` `dag.edge_reversed` (Phase 5), `6`
/// `post.route_fallback` (Phase 8).
pub const RESERVED_NOTE_CODES: [u32; 3] = [4, 5, 6];

/// The index of a note about the whole snapshot rather than one edge: `u32::MAX`, on the
/// JSON face the literal `4294967295`.
pub const SNAPSHOT_WIDE: u32 = u32::MAX;

/// Why a note code was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteCodeError {
    /// The code is allocated to a later phase that this reader does not implement.
    Reserved(u32),
    /// The code is not allocated to anything.
    Unknown(u32),
}

impl fmt::Display for NoteCodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reserved(code) => write!(f, "note code {code} is reserved and not implemented"),
            Self::Unknown(code) => write!(f, "note code {code} is not allocated"),
        }
    }
}

impl NoteCode {
    /// Every implemented code, ascending.
    pub const ALL: [Self; 3] = [
        Self::CycleEdgeDropped,
        Self::ExtraParentDropped,
        Self::PackingApproximate,
    ];

    /// The code as it goes on the wire.
    pub const fn code(self) -> u32 {
        self as u32
    }

    /// Reads a code, telling a reserved one apart from an unallocated one.
    pub const fn from_code(code: u32) -> Result<Self, NoteCodeError> {
        match code {
            1 => Ok(Self::CycleEdgeDropped),
            2 => Ok(Self::ExtraParentDropped),
            3 => Ok(Self::PackingApproximate),
            4..=6 => Err(NoteCodeError::Reserved(code)),
            other => Err(NoteCodeError::Unknown(other)),
        }
    }

    /// Its dotted name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::CycleEdgeDropped => "hierarchy.cycle_edge_dropped",
            Self::ExtraParentDropped => "hierarchy.extra_parent_dropped",
            Self::PackingApproximate => "packing.approximate",
        }
    }
}

/// One note, as a producer states it. Ordered by `(code, index)`: the canonical order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Note {
    /// What happened.
    pub code: NoteCode,
    /// Where: an edge position, or [`SNAPSHOT_WIDE`].
    pub index: u32,
}

/// The notes section as its two columns, as read or about to be written; whether they
/// hold a valid, canonical set is [`Snapshot::new`](crate::binary::Snapshot::new)'s to say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Notes {
    /// Each note's code.
    pub code: Vec<u32>,
    /// Each note's index.
    pub index: Vec<u32>,
}

impl Notes {
    /// The columns of `notes`, in the order given: sort them first, the constructor
    /// refuses them otherwise.
    pub fn of(notes: &[Note]) -> Self {
        Self {
            code: notes.iter().map(|n| n.code.code()).collect(),
            index: notes.iter().map(|n| n.index).collect(),
        }
    }

    /// Number of notes, `k`.
    pub fn len(&self) -> u32 {
        crate::geometry::index_u32(self.code.len())
    }

    /// Whether there is none.
    pub fn is_empty(&self) -> bool {
        self.code.is_empty() && self.index.is_empty()
    }

    /// `Ok` when the columns are as long as each other, the version carries notes if
    /// there are any, and every note is implemented, points where its code allows and
    /// comes strictly after the one before it.
    pub(crate) fn check(&self, version: FormatVersion, m: u32) -> Result<(), SnapshotError> {
        crate::geometry::check_len("note.index", u64::from(self.len()), self.index.len())?;
        if !self.is_empty() && !carries_notes(version) {
            return Err(SnapshotError::NotesUnsupported { version });
        }
        let mut previous = None;
        for (i, (&code, &at)) in (0..).zip(self.code.iter().zip(&self.index)) {
            let note = NoteCode::from_code(code)
                .map_err(|error| SnapshotError::NoteCode { index: i, error })?;
            let allowed = match note {
                NoteCode::PackingApproximate => at == SNAPSHOT_WIDE,
                _ => at < m,
            };
            if !allowed {
                return Err(SnapshotError::NoteTarget { index: i });
            }
            if previous.is_some_and(|before| before >= (code, at)) {
                return Err(SnapshotError::NoteOrder { index: i });
            }
            previous = Some((code, at));
        }
        Ok(())
    }
}

/// Whether a snapshot of `version` has a notes section: format 0.3 and every later minor
/// of major 0. The binary decoder reads the section only then, and treats `k` as 0
/// otherwise — decided by the version, never by sniffing for bytes at the end.
pub const fn carries_notes(version: FormatVersion) -> bool {
    version.major == 0 && version.minor >= 3
}

#[cfg(test)]
mod tests;
