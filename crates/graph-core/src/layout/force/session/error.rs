//! Why a session said no. Every variant names the thing that was wrong, because the whole
//! point of refusing rather than clamping is that the caller can act on the answer.

use crate::arena::CapacityError;
use crate::stage::StageError;
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
    /// A column's length is not the row count it is read against: a position column
    /// against the session's topology, the `from` of a
    /// [`carry`](super::ForceSession::carry) against the session's own rows, or the
    /// topology of a [`grow`](super::ForceSession::grow) holding fewer nodes than the
    /// session's rows or fewer edges than it absorbed.
    ColumnLength {
        /// Which column: `xs`, `ys`, `from` — the topology a carry is leaving — or `grow`.
        column: &'static str,
        /// How many values it held: for `grow`, the topology's nodes or edges.
        got: u64,
        /// How many nodes the topology has: for `grow`, the rows or edges the session holds.
        nodes: u32,
    },
    /// A [`NodeRow`](super::NodeRow) past the last column.
    NoSuchRow {
        /// The row asked for.
        row: u32,
        /// How many rows there are.
        rows: u32,
    },
    /// A [`grow`](super::ForceSession::grow) whose new edges could overflow the session's
    /// `u32` adjacency.
    Capacity(CapacityError),
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
            Self::Capacity(err) => err.fmt(f),
        }
    }
}

impl From<SessionError> for StageError {
    /// The frozen stage is a session with the frozen parameters, so a refusal to build one
    /// is a refusal to run the stage. `OutOfRange` carries its rule as a `&'static str`
    /// precisely so this conversion loses nothing.
    ///
    /// The other two variants cannot arise on the frozen path at all — a stage builds its
    /// own session and addresses no row — and `StageError::Param`'s rule is a
    /// `&'static str` with nowhere to format the numbers into. The numbers are in
    /// [`SessionError`], which is what a session's own caller sees and what its `Display`
    /// prints.
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::NonFinite { field } => Self::NonFinite { column: field },
            SessionError::OutOfRange { field, rule } => Self::Param { name: field, rule },
            SessionError::ColumnLength { .. } => Self::Param {
                name: "force session columns",
                rule: "one finite value per node",
            },
            SessionError::NoSuchRow { .. } => Self::Param {
                name: "force session row",
                rule: "a row inside the node columns",
            },
            SessionError::Capacity(err) => Self::Capacity(err),
        }
    }
}
