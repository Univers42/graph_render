//! How a [`SnapshotError`] reads: the column and, where there is one, the position, in
//! every message, so a refusal points at the byte that caused it. Split from `error.rs`
//! for the house function line limit. One exhaustive match names every variant, three
//! writers hold the message shapes most of them share, and a test pins every string byte
//! for byte, so the split cannot move a byte of a refusal.

use super::SnapshotError;
use crate::version::FormatVersion;
use core::fmt;

/// The fault texts too long to sit in an arm without pushing the match over the house
/// function line limit. Each is pinned byte for byte by the tests below.
const OFFSETS: &str = "offsets must start at 0, never decrease and end at the data's end";
const CURVE_DEGREE: &str = "edge.degree: a curve needs degree 1 or more";
const NOTE_ORDER: &str = "notes must be strictly ascending by (code, index)";
const DIM_UNNAMEABLE: &str = "names no dimension; a z column needs 0.4 or later";
const NOTES_UNSUPPORTED: &str = "carries none; notes need 0.3 or later";

impl fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        message(*self, f)
    }
}

/// Every variant's message, in one match and with no `_` arm: a variant added to
/// [`SnapshotError`] lands here and fails to compile rather than printing an empty
/// refusal. Most of the arms are one of three shapes — `column[index]: fault`,
/// `column: fault`, `column: format {version} consequence` — so the strings they share
/// live in the writers below.
fn message(error: SnapshotError, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match error {
        SnapshotError::Header(err) => fmt::Display::fmt(&err, f),
        SnapshotError::TrailingBytes { count } => write!(f, "{count} bytes after the last column"),
        SnapshotError::CurveDegree => write!(f, "{CURVE_DEGREE}"),
        SnapshotError::NoteCode { index, error } => write!(f, "note.code[{index}]: {error}"),
        SnapshotError::Length {
            column,
            expected,
            found,
        } => write!(f, "{column}: {found} values, need {expected}"),
        SnapshotError::NotesUnsupported { version } => {
            at_ver(f, "notes", version, NOTES_UNSUPPORTED)
        }
        SnapshotError::DimUnnameable { version } => at_ver(f, "node.z", version, DIM_UNNAMEABLE),
        SnapshotError::Truncated { column } => alone(f, column, "the snapshot ends inside it"),
        SnapshotError::Padding { column } => alone(f, column, "padding bytes must be 0"),
        SnapshotError::Capacity { column } => alone(f, column, "more than a u32 can count"),
        SnapshotError::NonFinite { column, index } => at(f, column, index, "NaN or infinite"),
        SnapshotError::Negative { column, index } => at(f, column, index, "negative"),
        SnapshotError::Utf8 { column, index } => at(f, column, index, "not UTF-8"),
        SnapshotError::Endpoint { column, index } => at(f, column, index, "not a node"),
        SnapshotError::DuplicateId { column, index } => {
            at(f, column, index, "repeats an earlier id")
        }
        SnapshotError::Offsets { column, index } => at(f, column, index, OFFSETS),
        SnapshotError::NoteOrder { index } => at(f, "note", index, NOTE_ORDER),
        SnapshotError::NoteTarget { index } => {
            at(f, "note.index", index, "not an index its code allows")
        }
    }
}

/// `column[index]: fault` — the shape every value, id and topology fault shares.
fn at(f: &mut fmt::Formatter<'_>, column: &str, index: u32, fault: &str) -> fmt::Result {
    write!(f, "{column}[{index}]: {fault}")
}

/// `column: fault` — the shape a fault with no single position wears.
fn alone(f: &mut fmt::Formatter<'_>, column: &str, fault: &str) -> fmt::Result {
    write!(f, "{column}: {fault}")
}

/// `column: format {version} consequence`.
fn at_ver(
    f: &mut fmt::Formatter<'_>,
    column: &str,
    version: FormatVersion,
    consequence: &str,
) -> fmt::Result {
    write!(f, "{column}: format {version} {consequence}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notes::NoteCodeError;
    use crate::snapshot::ReadError;
    use crate::version::FormatVersion;

    /// Byte for byte, not as a substring: the contract pins the text of a refusal, so a
    /// byte added anywhere in it is a change even though `contains` would still pass.
    fn assert_pinned(cases: impl IntoIterator<Item = (SnapshotError, &'static str)>) {
        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }

    fn version() -> FormatVersion {
        FormatVersion { major: 0, minor: 3 }
    }

    #[test]
    fn every_header_and_length_refusal_reads_exactly_what_the_contract_pins() {
        assert_pinned([
            (
                SnapshotError::Header(ReadError::BadMagic),
                "not a graph-motor snapshot: bad magic",
            ),
            (
                SnapshotError::Truncated { column: "node.x" },
                "node.x: the snapshot ends inside it",
            ),
            (
                SnapshotError::TrailingBytes { count: 3 },
                "3 bytes after the last column",
            ),
            (
                SnapshotError::Length {
                    column: "edge.pts",
                    expected: 4,
                    found: 2,
                },
                "edge.pts: 2 values, need 4",
            ),
        ]);
    }

    #[test]
    fn every_value_refusal_reads_exactly_what_the_contract_pins() {
        assert_pinned([
            (
                SnapshotError::NonFinite {
                    column: "node.y",
                    index: 5,
                },
                "node.y[5]: NaN or infinite",
            ),
            (
                SnapshotError::Negative {
                    column: "node.r",
                    index: 1,
                },
                "node.r[1]: negative",
            ),
            (
                SnapshotError::Offsets {
                    column: "node.id",
                    index: 2,
                },
                "node.id[2]: offsets must start at 0, never decrease and end at the data's end",
            ),
            (
                SnapshotError::Utf8 {
                    column: "edge.id",
                    index: 0,
                },
                "edge.id[0]: not UTF-8",
            ),
            (
                SnapshotError::Padding { column: "node.id" },
                "node.id: padding bytes must be 0",
            ),
        ]);
    }

    #[test]
    fn every_id_and_topology_refusal_reads_exactly_what_the_contract_pins() {
        assert_pinned([
            (
                SnapshotError::DuplicateId {
                    column: "node.id",
                    index: 9,
                },
                "node.id[9]: repeats an earlier id",
            ),
            (
                SnapshotError::Endpoint {
                    column: "edge.target",
                    index: 4,
                },
                "edge.target[4]: not a node",
            ),
            (
                SnapshotError::CurveDegree,
                "edge.degree: a curve needs degree 1 or more",
            ),
            (
                SnapshotError::Capacity { column: "edge.id" },
                "edge.id: more than a u32 can count",
            ),
        ]);
    }

    #[test]
    fn every_note_and_version_refusal_reads_exactly_what_the_contract_pins() {
        assert_pinned([
            (
                SnapshotError::NoteCode {
                    index: 2,
                    error: NoteCodeError::Reserved(7),
                },
                "note.code[2]: note code 7 is reserved and not implemented",
            ),
            (
                SnapshotError::NoteOrder { index: 6 },
                "note[6]: notes must be strictly ascending by (code, index)",
            ),
            (
                SnapshotError::NoteTarget { index: 8 },
                "note.index[8]: not an index its code allows",
            ),
            (
                SnapshotError::NotesUnsupported { version: version() },
                "notes: format 0.3 carries none; notes need 0.3 or later",
            ),
            (
                SnapshotError::DimUnnameable { version: version() },
                "node.z: format 0.3 names no dimension; a z column needs 0.4 or later",
            ),
        ]);
    }
}
