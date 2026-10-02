//! The one edge-strength table, by edge kind
//! (`docs/decisions/edge-strength-table.md` is the record and the citations).
//!
//! Graph derivation existed in three copies that had already diverged, so **two live
//! code paths produced different layouts for the same data**. This module is the one
//! implementation: an exhaustive `match`, total over [`EdgeKind`], read by every
//! derivation in the motor. A kind the match does not name is a compile error rather
//! than a silent default, and the table below is **not** a second copy of the numbers:
//! every row reads its value out of [`edge_strength`], so there is one place a strength
//! is written down.

use crate::edgekind::EdgeKind;

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
    // the table scan it replaced could not deliver. This match is the only place a strength
    // is written down; `STRENGTH_TABLE` reads it rather than repeating it, and the order of
    // the table's rows is a reading order, not a second source of the values.
    match kind {
        EdgeKind::Hierarchy => 2.0,
        EdgeKind::Relation => 1.0,
        EdgeKind::Tag => 0.75,
        EdgeKind::NoteLink => 0.625,
        EdgeKind::NoteOf => 0.5,
    }
}

/// Every kind with its strength, strongest first: a hierarchy edge is the structure the
/// drawing is built on, an ordinary relation is the reference weight, and the annotation
/// kinds pull progressively less.
///
/// The rows name the kinds in that order and **borrow every value from
/// [`edge_strength`]**, so this is a reading order over one source rather than a second
/// copy of the five numbers. `edge_strength` is a `const fn`, which is what lets a
/// `const` array be a view of the match at all.
///
/// Every value is a **dyadic rational** (a multiple of 2⁻⁶), so it is exact in `f64`
/// natively and in `f32` on the transport face, and exact through the JSON face's
/// `f32 → shortest decimal → f64 → f32` round-trip. No strength in this table can
/// differ by one ULP between targets, which is what D9 asks for.
pub const STRENGTH_TABLE: [(EdgeKind, f64); 5] = [
    (EdgeKind::Hierarchy, edge_strength(EdgeKind::Hierarchy)),
    (EdgeKind::Relation, edge_strength(EdgeKind::Relation)),
    (EdgeKind::Tag, edge_strength(EdgeKind::Tag)),
    (EdgeKind::NoteLink, edge_strength(EdgeKind::NoteLink)),
    (EdgeKind::NoteOf, edge_strength(EdgeKind::NoteOf)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_pins_every_kind_to_its_exact_value() {
        // The absolute numbers, spelled out: `docs/decisions/edge-strength-table.md` is
        // the record of the choice, and this is the test that says the code still is it.
        assert_eq!(edge_strength(EdgeKind::Hierarchy), 2.0);
        assert_eq!(edge_strength(EdgeKind::Relation), 1.0);
        assert_eq!(edge_strength(EdgeKind::Tag), 0.75);
        assert_eq!(edge_strength(EdgeKind::NoteLink), 0.625);
        assert_eq!(edge_strength(EdgeKind::NoteOf), 0.5);
    }

    #[test]
    fn the_table_states_no_strength_the_match_does_not() {
        // Every row's value is `edge_strength`'s own, read rather than written: if the
        // match changes, this changes with it, and the two cannot drift.
        for (kind, value) in STRENGTH_TABLE {
            assert_eq!(value, edge_strength(kind), "{kind:?}");
        }
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
    fn no_two_kinds_share_a_value_so_a_swapped_case_fails() {
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
