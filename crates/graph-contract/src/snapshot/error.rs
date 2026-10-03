//! Every reason a snapshot is refused, by either face or at construction. Split from
//! `snapshot.rs` for the house line limit; re-exported there as `SnapshotError`.

use super::ReadError;
use crate::notes::NoteCodeError;
use crate::version::FormatVersion;

mod display;

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
