//! The one edge-strength table, by edge kind
//! (`docs/decisions/edge-strength-table.md` is the record and the citations).
//!
//! Graph derivation existed in three copies that had already diverged, so **two live
//! code paths produced different layouts for the same data**. This module is the one
//! implementation: a constant table, total over [`EdgeKind::ALL`], read by every
//! derivation in the motor. A kind that is not in the table has no strength, which is a
//! compile error rather than a silent default.

use crate::edgekind::EdgeKind;

/// The strength every kind gets, strongest first: a hierarchy edge is the structure
/// the drawing is built on, an ordinary relation is the reference weight, and the
/// annotation kinds pull progressively less.
///
/// Every value is a **dyadic rational** (a multiple of 2⁻⁶), so it is exact in `f64`
/// natively and in `f32` on the transport face, and exact through the JSON face's
/// `f32 → shortest decimal → f64 → f32` round-trip. No strength in this table can
/// differ by one ULP between targets, which is what D9 asks for.
pub const STRENGTH_TABLE: [(EdgeKind, f64); 5] = [
    (EdgeKind::Hierarchy, 2.0),
    (EdgeKind::Relation, 1.0),
    (EdgeKind::Tag, 0.75),
    (EdgeKind::NoteLink, 0.625),
    (EdgeKind::NoteOf, 0.5),
];

/// The strength for one edge kind: its one row in [`STRENGTH_TABLE`].
///
/// Ponytail: **a chosen convention, not a derived truth.** No measurement says an
/// annotation should pull 0.625; the ordering is an argument about which facts should
/// dominate a layout, and the magnitudes are round numbers under it. The failure mode
/// is silent and global: change a value and *every* layout changes, so every pinned
/// snapshot hash and every recorded geometry measurement from here on is stale, with
/// nothing in the output saying so. Direction: the table is a convention precisely
/// because two existing implementations disagreed about it (1.2/1.8/0.7/0.5 against
/// 3.2/1.4) and neither could be shown wrong from geometry alone — the choice is
/// reviewable, not derivable. Escape hatch: this function is the only reader, so a
/// caller that needs a different convention overrides the edge's own `strength` after
/// [`crate::index::index_model`] rather than editing this table.
pub const fn edge_strength(kind: EdgeKind) -> f64 {
    // An exhaustive `match`, so a new `EdgeKind` variant is a **compile error** rather than
    // a runtime panic in a release build — which is what this module's doc claims, and what
    // the table scan it replaced could not deliver. `STRENGTH_TABLE` stays the one
    // documented table and the totality test below is what keeps the two from drifting.
    match kind {
        EdgeKind::Hierarchy => 2.0,
        EdgeKind::Relation => 1.0,
        EdgeKind::Tag => 0.75,
        EdgeKind::NoteLink => 0.625,
        EdgeKind::NoteOf => 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pins_every_kind_to_its_exact_value() {
        assert_eq!(edge_strength(EdgeKind::Hierarchy), 2.0);
        assert_eq!(edge_strength(EdgeKind::Relation), 1.0);
        assert_eq!(edge_strength(EdgeKind::Tag), 0.75);
        assert_eq!(edge_strength(EdgeKind::NoteLink), 0.625);
        assert_eq!(edge_strength(EdgeKind::NoteOf), 0.5);
    }

    #[test]
    fn the_table_is_total_over_all_kinds() {
        assert_eq!(STRENGTH_TABLE.len(), EdgeKind::ALL.len());
        for kind in EdgeKind::ALL {
            let row = STRENGTH_TABLE
                .iter()
                .find(|(k, _)| *k == kind)
                .unwrap_or_else(|| panic!("{kind:?} has no row"));
            assert_eq!(row.1, edge_strength(kind));
        }
    }

    #[test]
    fn no_two_kinds_share_a_value_so_a_swapped_row_fails() {
        let mut values: Vec<u64> = STRENGTH_TABLE.iter().map(|(_, v)| v.to_bits()).collect();
        values.sort_unstable();
        let before = values.len();
        values.dedup();
        assert_eq!(before, values.len(), "two kinds share a strength");
    }

    #[test]
    fn the_ladder_is_strictly_descending_in_discriminant_order() {
        let mut previous = f64::INFINITY;
        for (kind, value) in STRENGTH_TABLE {
            assert!(
                value < previous,
                "{kind:?} at {value} is not below the previous row's {previous}"
            );
            previous = value;
        }
    }

    #[test]
    fn every_value_is_a_dyadic_rational_and_survives_the_json_face() {
        for (kind, value) in STRENGTH_TABLE {
            assert!(
                value == f64::from(value as f32),
                "{kind:?} at {value} does not survive f64 → f32 → f64"
            );
            assert!(value.is_finite() && value > 0.0, "{kind:?} at {value}");
            // The shortest decimal Rust prints, re-read as f64, must be the same bits.
            assert_eq!(
                format!("{value}").parse::<f64>().unwrap().to_bits(),
                value.to_bits()
            );
        }
    }
}
