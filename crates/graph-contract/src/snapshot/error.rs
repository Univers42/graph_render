//! Every reason a snapshot is refused, by either face or at construction. Split from
//! `snapshot.rs` for the house line limit; re-exported there as `SnapshotError`.

use super::ReadError;
use crate::notes::NoteCodeError;
use crate::version::FormatVersion;
use core::fmt;

/// Why a snapshot was refused, by either face or at construction. Every variant names
/// the column (`node.id`, `edge.source`, `node.x`, `edge.pts`, …) and, where there is one,
/// the position that broke the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    /// The fixed header was refused.
    Header(ReadError),
    /// The payload ends inside a column the header's counts call for.
    Truncated {
        /// The column that was cut short.
        column: &'static str,
    },
    /// Bytes remain after the last column.
    TrailingBytes {
        /// How many.
        count: u64,
    },
    /// A column's length is not the one the counts require.
    Length {
        /// The column.
        column: &'static str,
        /// Values it must hold.
        expected: u64,
        /// Values it holds.
        found: u64,
    },
    /// NaN or ±∞, which may not be hashed (D9).
    NonFinite {
        /// The column.
        column: &'static str,
        /// The first bad position.
        index: u32,
    },
    /// A negative radius, width or height.
    Negative {
        /// The column.
        column: &'static str,
        /// The first bad position.
        index: u32,
    },
    /// Offsets that do not start at 0, decrease, or run past their data.
    Offsets {
        /// The column.
        column: &'static str,
        /// The first bad offset.
        index: u32,
    },
    /// An id whose bytes are not UTF-8.
    Utf8 {
        /// The column.
        column: &'static str,
        /// The id's position.
        index: u32,
    },
    /// A padding byte that is not 0.
    Padding {
        /// The column the padding closes.
        column: &'static str,
    },
    /// An id equal to an earlier one in the same table.
    DuplicateId {
        /// The column.
        column: &'static str,
        /// The later of the two positions.
        index: u32,
    },
    /// An edge endpoint that is not a node of this snapshot.
    Endpoint {
        /// `edge.source` or `edge.target`.
        column: &'static str,
        /// The edge.
        index: u32,
    },
    /// A curve of degree 0.
    CurveDegree,
    /// More ids, id bytes or points than a `u32` counts.
    Capacity {
        /// The column.
        column: &'static str,
    },
    /// A note code this reader refuses: reserved for a later phase, or not allocated.
    NoteCode {
        /// The note's position.
        index: u32,
        /// Which.
        error: NoteCodeError,
    },
    /// A note not strictly after the one before it by `(code, index)`: a repeat, or
    /// out of order.
    NoteOrder {
        /// The later note.
        index: u32,
    },
    /// A note index its code does not allow: an edge position `≥ m` for codes 1-2,
    /// anything but `u32::MAX` for code 3.
    NoteTarget {
        /// The note's position.
        index: u32,
    },
    /// Notes in a snapshot whose format predates them (below 0.3).
    NotesUnsupported {
        /// The format it declares.
        version: FormatVersion,
    },
    /// A z column in a snapshot whose format predates `dim` (below 0.4), so the dimension
    /// the bytes carry has no name in the version they claim.
    DimUnnameable {
        /// The format it declares.
        version: FormatVersion,
    },
}

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Header(err) => err.fmt(f),
            Self::Truncated { column } => write!(f, "{column}: the snapshot ends inside it"),
            Self::TrailingBytes { count } => write!(f, "{count} bytes after the last column"),
            Self::Length {
                column,
                expected,
                found,
            } => write!(f, "{column}: {found} values, need {expected}"),
            Self::NonFinite { column, index } => write!(f, "{column}[{index}]: NaN or infinite"),
            Self::Negative { column, index } => write!(f, "{column}[{index}]: negative"),
            Self::Offsets { column, index } => {
                write!(
                    f,
                    "{column}[{index}]: offsets must start at 0, never decrease and end at the data's end"
                )
            }
            Self::Utf8 { column, index } => write!(f, "{column}[{index}]: not UTF-8"),
            Self::Padding { column } => write!(f, "{column}: padding bytes must be 0"),
            Self::DuplicateId { column, index } => {
                write!(f, "{column}[{index}]: repeats an earlier id")
            }
            Self::Endpoint { column, index } => write!(f, "{column}[{index}]: not a node"),
            Self::CurveDegree => write!(f, "edge.degree: a curve needs degree 1 or more"),
            Self::Capacity { column } => write!(f, "{column}: more than a u32 can count"),
            Self::NoteCode { index, error } => write!(f, "note.code[{index}]: {error}"),
            Self::NoteOrder { index } => write!(
                f,
                "note[{index}]: notes must be strictly ascending by (code, index)"
            ),
            Self::NoteTarget { index } => {
                write!(f, "note.index[{index}]: not an index its code allows")
            }
            Self::NotesUnsupported { version } => write!(
                f,
                "notes: format {version} carries none; notes need 0.3 or later"
            ),
            Self::DimUnnameable { version } => write!(
                f,
                "node.z: format {version} names no dimension; a z column needs 0.4 or later"
            ),
        }
    }
}
