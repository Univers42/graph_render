//! The one pass over the columns that decides whether the *values* are readable, run after
//! the section table has proved the bytes add up.
//!
//! It is a separate pass from `row()` on purpose: everything here is checked exactly once,
//! against the whole document, so `row()` may then be total for every row the header counts
//! and need no `Option`, no `expect` and no bounds guess of its own.

use super::Columns;
use super::Format;
use super::error::ColumnsError;
use super::layout::{ABSENT, Shape};

/// A `u32` cell at `row`. The section table proved this column is `4 * count` long, so the
/// read cannot leave it.
pub(super) fn cell(column: &[u8], row: u32) -> u32 {
    let at = 4 * row as usize;
    u32::from_le_bytes(column[at..at + 4].try_into().expect("4 bytes"))
}

/// Row `row` of an `f64` column, from its eight little-endian bytes. Never a pointer cast:
/// the pad keeps the columns eight-aligned so a `&[f64]` *would* be legal, but taking one is
/// `unsafe` and reading the bits is not.
pub(super) fn float(column: &[u8], row: u32) -> f64 {
    let at = 8 * row as usize;
    f64::from_bits(u64::from_le_bytes(
        column[at..at + 8].try_into().expect("8 bytes"),
    ))
}

/// The `u32` in cell `cell` of `column`, or `None` if the column has no such cell. Used by
/// `ColumnsDoc::text`, which is handed a string index by a host and must not read one past
/// the offset table to find out. Checked, so a cell past `usize::MAX / 4` is `None` rather
/// than a wrapped index into cell 0 (a release build on wasm32 does not trap on overflow).
pub(super) fn cell_at(column: &[u8], cell: usize) -> Option<u32> {
    let at = cell.checked_mul(4)?;
    let four: [u8; 4] = column.get(at..at.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(four))
}

/// Refuses every value the contract does not allow: a string index that names nothing, a
/// `u32::MAX` in a column where absent is not a meaning, an endpoint past the last node, a
/// boolean that is not `0` or `1`, and a float that is not finite.
pub(super) fn check(columns: &Columns<'_>, shape: Shape) -> Result<(), ColumnsError> {
    for row in 0..node_count(shape) {
        required(columns.id, "node id", row, shape)?;
        required(columns.kind, "node kind", row, shape)?;
        required(columns.source, "node source", row, shape)?;
        required(columns.label, "node label", row, shape)?;
        optional(columns.database, "node database", row, shape)?;
        optional(columns.group, "node group", row, shape)?;
        optional(columns.icon, "node icon", row, shape)?;
        boolean(columns.has_note, "node has_note", row)?;
    }
    for row in 0..edge_count(shape) {
        required(columns.edge_id, "edge id", row, shape)?;
        required(columns.edge_kind, "edge kind", row, shape)?;
        required(columns.edge_label, "edge label", row, shape)?;
        optional(columns.record_id, "edge record_id", row, shape)?;
        endpoint(columns.edge_source, "edge source", row, shape)?;
        endpoint(columns.edge_target, "edge target", row, shape)?;
        boolean(columns.directed, "edge directed", row)?;
        boolean(columns.child_first, "edge child_first", row)?;
    }
    finite(columns.weight, "node weight", node_count(shape))?;
    finite(columns.version, "node version", node_count(shape))?;
    finite(columns.strength, "edge strength", edge_count(shape))?;
    Ok(())
}

/// `offsets` starts at `0`, never decreases, ends at the blob length, and every entry's byte
/// range is a whole `str`. The ranges are cut with `str::get`, never by asserting a slice
/// is UTF-8: the blob is valid, its *sub-ranges* need not be.
pub(super) fn check_table(offsets: &[u8], blob: &str, count: u32) -> Result<(), ColumnsError> {
    if cell(offsets, 0) != 0 {
        return Err(ColumnsError::OffsetOrigin {
            found: cell(offsets, 0),
        });
    }
    if cell(offsets, count) != blob.len() as u32 {
        return Err(ColumnsError::OffsetEnd {
            found: cell(offsets, count),
            blob_len: blob.len() as u32,
        });
    }
    // `0..count`, not `0..=count`: entry `count - 1`'s range ends at the closing offset,
    // which the closing-offset check above has already compared with the blob length.
    for index in 0..count {
        let (lo, hi) = (cell(offsets, index), cell(offsets, index + 1));
        if hi < lo {
            return Err(ColumnsError::DecreasingOffset { index });
        }
        if hi > blob.len() as u32 {
            return Err(ColumnsError::OffsetOutOfRange { index });
        }
        if blob.get(lo as usize..hi as usize).is_none() {
            return Err(ColumnsError::SplitCodePoint { index });
        }
    }
    Ok(())
}

/// The node and edge row counts, read back off the buffer.
fn node_count(shape: Shape) -> u32 {
    u32::try_from(shape.nodes).expect("a count below u32::MAX")
}

fn edge_count(shape: Shape) -> u32 {
    u32::try_from(shape.edges).expect("a count below u32::MAX")
}

/// A string index that must name a string: below `string_count`, never the absent marker.
fn required(column: &[u8], name: &'static str, row: u32, shape: Shape) -> Result<(), ColumnsError> {
    let value = cell(column, row);
    if value < shape.strings as u32 {
        return Ok(());
    }
    Err(if value == ABSENT {
        ColumnsError::RequiredAbsent { column: name, row }
    } else {
        ColumnsError::StringIndex { column: name, row }
    })
}

/// A string index where absent is a meaning: below `string_count`, or `u32::MAX`.
fn optional(column: &[u8], name: &'static str, row: u32, shape: Shape) -> Result<(), ColumnsError> {
    let value = cell(column, row);
    if value < shape.strings as u32 || value == ABSENT {
        return Ok(());
    }
    Err(ColumnsError::StringIndex { column: name, row })
}

/// An edge endpoint, which names a node in the only way this format's magic allows: a
/// **node row number** under `GMC1`, a **string index naming a node id** under `GMX1`.
///
/// One function with two arms, not two checkers, because every other rule above is shared
/// and a second file would be a second copy of them. The batch arm is [`required`]: a batch
/// endpoint may name a node the graph already holds, so it is a name like any other — never
/// `u32::MAX`, and an entry past the table names nothing.
fn endpoint(column: &[u8], name: &'static str, row: u32, shape: Shape) -> Result<(), ColumnsError> {
    match shape.format {
        Format::Batch => required(column, name, row, shape),
        Format::Document => match cell(column, row) < shape.nodes as u32 {
            true => Ok(()),
            false => Err(ColumnsError::EndpointRow { column: name, row }),
        },
    }
}

/// `0` or `1`, nothing else. A `2` is refused rather than read as "true": the contract says
/// the cell is a boolean, and a producer that wrote `2` has a bug this surfaces.
fn boolean(column: &[u8], name: &'static str, row: u32) -> Result<(), ColumnsError> {
    match cell(column, row) {
        0 | 1 => Ok(()),
        found => Err(ColumnsError::NotBoolean {
            column: name,
            row,
            found,
        }),
    }
}

/// Every cell of `count` rows is finite. `-0.0` and a subnormal pass: both are ordinary
/// finite `f64`s and `ingest::read_records` accepts them too, so refusing them here would
/// make the differential refuse a document the other path takes.
fn finite(column: &[u8], name: &'static str, count: u32) -> Result<(), ColumnsError> {
    for row in 0..count {
        if !float(column, row).is_finite() {
            return Err(ColumnsError::NotFinite { column: name, row });
        }
    }
    Ok(())
}
