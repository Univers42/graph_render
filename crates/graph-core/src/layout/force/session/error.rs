//! Why a session said no. Every variant names the thing that was wrong, because the whole
//! point of refusing rather than clamping is that the caller can act on the answer.

use core::fmt;

/// Why a [`ForceSession`](super::ForceSession) could not be built, stepped onto, or given
/// parameters.
///
/// The values are all `&'static str` or small integers rather than formatted text: the
/// motor has no allocator to spare on an error path (D6 — nothing `usize` reaches the
/// wire), and a `rule` that is a compile-time constant is one the frozen stage can hand
/// straight to `StageError::Param`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SessionError {
    /// A value was NaN or ±∞, whose bits wasm32 does not pin (D9).
    NonFinite {
        /// Which value: a parameter's name, or a position column.
        field: &'static str,
    },
    /// A value was finite and outside the range its field accepts.
    OutOfRange {
        /// The parameter.
        field: &'static str,
        /// The rule it broke, with the numbers in it.
        rule: &'static str,
    },
    /// A position column's length is not the topology's node count.
    ColumnLength {
        /// Which column, `xs` or `ys`.
        column: &'static str,
        /// How many values it held.
        got: u64,
        /// How many nodes the topology has.
        nodes: u32,
    },
    /// A [`NodeRow`](super::NodeRow) past the last column.
    NoSuchRow {
        /// The row asked for.
        row: u32,
        /// How many rows there are.
        rows: u32,
    },
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite { field } => write!(f, "non-finite value in {field}"),
            Self::OutOfRange { rule, .. } => write!(f, "parameter {rule}"),
            Self::ColumnLength { column, got, nodes } => {
                write!(f, "column {column}: {got} values for {nodes} nodes")
            }
            Self::NoSuchRow { row, rows } => write!(f, "row {row} of {rows}"),
        }
    }
}
