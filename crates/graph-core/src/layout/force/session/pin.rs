//! [`NodeRow`], the way a caller names a node: **its row in the snapshot columns**, in the
//! order the positions are read at.
//!
//! The dense index never leaves the motor (`prompt.md` §4, "Identity"), and this is how
//! that stays true while a caller still gets to say *this* node. Today a row is the dense
//! index — one topology, one column order, and `index_model` assigns rows in insertion
//! order — so the two are interchangeable inside the crate and only `NodeRow` crosses the
//! boundary. `session/tests/m1d.rs` pins the mapping node for node, so a change in the
//! order positions are read at cannot pass unnoticed.

use super::ForceSession;
use super::error::SessionError;

/// A node's row in the snapshot's `x`/`y` columns. A newtype over `u32` and never over
/// `usize`: wasm32 is 32-bit and native is 64-bit (D6), so a `usize` here would be a
/// target-dependent index the moment it crossed the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeRow(u32);

impl NodeRow {
    /// Row `row` of the snapshot's node columns. The value is not checked here — the
    /// topology is not known yet — and every operation that takes one refuses a row past
    /// its end rather than ignoring it.
    pub const fn new(row: u32) -> Self {
        Self(row)
    }

    /// The row as it is stored, for indexing a slice the session handed back:
    /// `session.xs()[row.as_u32() as usize]`.
    pub const fn as_u32(self) -> u32 {
        self.0
    }

    /// `row`, or [`SessionError::NoSuchRow`] against `rows` — the check every operation
    /// that takes a row starts with, so an out-of-range row is always the same refusal.
    fn checked(self, rows: u32) -> Result<u32, SessionError> {
        if self.0 < rows {
            Ok(self.0)
        } else {
            Err(SessionError::NoSuchRow { row: self.0, rows })
        }
    }
}

impl ForceSession {
    /// Holds `node` at `(x, y)` exactly, from the next tick on: d3's `node.fx`/`node.fy`,
    /// and its own tick tail honours them (`simulation.js:53-56`) by *placing* the node
    /// rather than integrating it, zeroing the velocity on the way.
    ///
    /// Refused, with the session untouched, when the row is past the last column or
    /// either coordinate is not finite (D9) — a pin that silently did nothing would be a
    /// drag the user cannot explain.
    pub fn pin(&mut self, node: NodeRow, x: f64, y: f64) -> Result<(), SessionError> {
        if !x.is_finite() {
            return Err(SessionError::NonFinite { field: "pin.x" });
        }
        if !y.is_finite() {
            return Err(SessionError::NonFinite { field: "pin.y" });
        }
        let row = node.checked(self.rows())?;
        self.sim.fx[row as usize] = Some(x);
        self.sim.fy[row as usize] = Some(y);
        Ok(())
    }

    /// Releases one node: it integrates again from the velocity it was placed with, which
    /// is zero (`simulation.js:54`), so it starts again from rest.
    pub fn unpin(&mut self, node: NodeRow) -> Result<(), SessionError> {
        let row = node.checked(self.rows())?;
        self.sim.fx[row as usize] = None;
        self.sim.fy[row as usize] = None;
        Ok(())
    }

    /// Releases every node at once — the difference between "let go of the node under the
    /// cursor" and "let go of everything", which a UI needs and which is otherwise a
    /// loop over rows the caller would have to guess the length of.
    pub fn unpin_all(&mut self) {
        self.sim.fx.iter_mut().for_each(|fx| *fx = None);
        self.sim.fy.iter_mut().for_each(|fy| *fy = None);
    }
}

#[cfg(test)]
mod tests {
    use super::NodeRow;

    #[test]
    fn a_row_is_the_u32_it_was_built_from() {
        let row = NodeRow::new(7);
        assert_eq!(row.as_u32(), 7);
        assert!(
            NodeRow::new(0) < NodeRow::new(1),
            "and rows order, for a UI's sake"
        );
    }
}
