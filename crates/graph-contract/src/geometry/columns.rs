//! The node columns in wire order, and the rules each one has to satisfy.
//!
//! Split out of `geometry.rs` by the house's 300-line limit, and because the wire order is
//! one thing with its own rules: which columns a snapshot has, in what order, and what a
//! value in each may be. The z column is the reason it is its own file — a 3D snapshot has
//! one and a 2D one does not, and every reader and writer has to agree about where it sits.

use super::{NodeGeometry, SnapshotError, check_finite, check_len, index_u32, node_column};

/// Every column with its wire name, in wire order, with the z column spliced in right
/// after `y` when the snapshot is 3D. Coordinates stay contiguous (`x, y, z`) and the
/// sizes shift one word along, which is why a reader takes column positions from `dim` in
/// the header rather than from fixed offsets (`docs/contract/binary-layout.md`).
pub(super) fn columns_dim<'a>(
    nodes: &'a NodeGeometry,
    z: Option<&'a [f32]>,
) -> Vec<(&'static str, &'a [f32])> {
    let mut out = Vec::with_capacity(nodes.columns().len() + 1);
    for (name, column) in nodes.columns() {
        out.push((name, column));
        if name == "y"
            && let Some(z) = z
        {
            out.push(("z", z));
        }
    }
    out
}

/// `Ok` when every column — the z column included, when `z` is given — has `n` finite
/// values and no size is negative. A z is a coordinate, so it may be negative; only `r`, `w`
/// and `h` are sizes.
pub(super) fn check(nodes: &NodeGeometry, n: u32, z: Option<&[f32]>) -> Result<(), SnapshotError> {
    for (name, column) in columns_dim(nodes, z) {
        // `columns_dim` spells out every arm, so an unknown name cannot arrive from
        // outside; falling back to the name the column was given keeps this total without
        // ever reporting a column the caller did not name, and without a refusal variant
        // that means "no such column".
        let column_name = node_column(name).unwrap_or(name);
        check_len(column_name, u64::from(n), column.len())?;
        check_finite(column_name, column)?;
        if matches!(name, "r" | "w" | "h")
            && let Some(bad) = column.iter().position(|v| *v < 0.0)
        {
            return Err(SnapshotError::Negative {
                column: column_name,
                index: index_u32(bad),
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
