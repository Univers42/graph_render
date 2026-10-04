//! Compressed sparse rows: `offsets` (rows + 1) and `values`, both `u32` (D6).
//!
//! Built in O(rows + pairs) by a counting sort that is **stable**: within a row, values
//! keep the order the pairs arrived in. The topology feeds pairs in edge order, so every
//! row lists its edges in the oracle's `Map` insertion order (`model.ts:41-42`). A CSR
//! sorted by value would be more cache-friendly and would break the differential.

use crate::arena::CapacityError;

mod append;
pub use append::AppendCsr;

/// The most bytes the `offsets` table of one adjacency may take: 1 GiB, the ceiling
/// [`crate::budget::QUADRATIC_BYTES_MAX`] puts on a quadratic table. It bounds `rows` at
/// 2²⁸ − 1, far above every graph the registry admits (the largest bench model is
/// [`crate::registry::MAX_BENCH_NODES`], 1 000 000).
///
/// Ponytail: a request, not a measurement — the check asks whether the table *would* fit
/// in 1 GiB, never whether this host has 1 GiB free, so the same graph is built or
/// refused on every machine (D4). Direction: `rows` past the ceiling is a
/// [`CapacityError`] where it used to be an abort inside the allocator. Escape hatch:
/// [`AppendCsr`] grows one row at a time and answers to
/// [`AppendCsr::SAFE_LIVE`] instead, for an adjacency that starts near the ceiling.
pub(crate) const TABLE_BYTES_MAX: u64 = 1 << 30;

/// `len` `u32` elements, or `None` when the table would pass [`TABLE_BYTES_MAX`]. Counted
/// in `u64` so a 32-bit `usize` cannot wrap the product, and so the answer is the same on
/// every target (D6, budget.rs).
fn table_len(len: u64) -> Option<usize> {
    len.checked_mul(size_of::<u32>() as u64)
        .filter(|bytes| *bytes <= TABLE_BYTES_MAX)
        .map(|bytes| bytes as usize / size_of::<u32>())
}

/// One adjacency, row `r`'s values being `values[offsets[r]..offsets[r + 1]]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Csr {
    offsets: Vec<u32>,
    values: Vec<u32>,
}

impl Default for Csr {
    fn default() -> Self {
        Self {
            offsets: vec![0],
            values: Vec::new(),
        }
    }
}

impl Csr {
    /// Builds `rows` rows from `(row, value)` pairs, keeping arrival order within a row.
    /// `pairs` is walked twice (count, then place), so it must yield the same sequence
    /// both times; a second walk that does not is a caller bug and panics. A row index
    /// `>= rows` is a caller bug and panics. A `rows` past [`TABLE_BYTES_MAX`] is
    /// [`CapacityError`], refused before anything is allocated.
    pub fn from_pairs<I>(rows: u32, pairs: I) -> Result<Self, CapacityError>
    where
        I: Iterator<Item = (u32, u32)> + Clone,
    {
        let overflow = CapacityError { what: "adjacency" };
        let width = table_len(u64::from(rows).checked_add(1).ok_or(overflow)?).ok_or(overflow)?;
        let mut offsets = vec![0u32; width];
        for (row, _) in pairs.clone() {
            assert!(row < rows, "row {row} of {rows}");
            let slot = &mut offsets[row as usize + 1];
            *slot = slot.checked_add(1).ok_or(overflow)?;
        }
        let mut total = 0u32;
        for slot in &mut offsets {
            total = total.checked_add(*slot).ok_or(overflow)?;
            *slot = total;
        }
        let mut cursor = offsets.clone();
        let mut values = vec![0u32; table_len(u64::from(total)).ok_or(overflow)?];
        let mut placed = 0usize;
        for (row, value) in pairs {
            let end = offsets[row as usize + 1];
            let at = &mut cursor[row as usize];
            assert!(
                *at < end && placed < values.len(),
                "a different sequence on the second pass"
            );
            values[*at as usize] = value;
            *at += 1;
            placed += 1;
        }
        assert_eq!(
            placed,
            values.len(),
            "a different sequence on the second pass"
        );
        Ok(Self { offsets, values })
    }

    /// Row `row`'s values, in arrival order.
    pub fn row(&self, row: u32) -> &[u32] {
        let (start, end) = (self.offsets[row as usize], self.offsets[row as usize + 1]);
        &self.values[start as usize..end as usize]
    }

    /// Number of rows.
    pub fn rows(&self) -> u32 {
        (self.offsets.len() - 1) as u32
    }

    /// Total values across every row.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// True when no row holds a value.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Bytes held by `offsets` and `values`.
    pub fn byte_len(&self) -> usize {
        (self.offsets.len() + self.values.len()) * size_of::<u32>()
    }
}

/// Two ascending rows merged into one ascending sequence, a value present in both
/// yielded twice — behind [`Topology::incident`](crate::Topology::incident).
#[derive(Debug, Clone)]
pub struct Incident<'a> {
    out: &'a [u32],
    inbound: &'a [u32],
}

impl<'a> Incident<'a> {
    /// Merges `out` and `inbound`, each ascending.
    ///
    /// # Precondition
    ///
    /// Both rows ascending. [`Csr::from_pairs`] keeps *arrival* order within a row
    /// (`csr.rs:3-6`), so a row built from unsorted pairs is **not** ascending and the
    /// merge below is not a merge: it emits the two rows interleaved by first element, in
    /// no order at all. [`Topology`](crate::Topology) never hands such a row over — its
    /// out/inbound tables are filled by ascending edge index (`index.rs`) — so a
    /// violation is a caller bug, caught in a debug build rather than answered with
    /// silently wrong incident lists.
    pub fn merge(out: &'a [u32], inbound: &'a [u32]) -> Self {
        debug_assert!(
            out.is_sorted() && inbound.is_sorted(),
            "Incident::merge needs ascending rows"
        );
        Self { out, inbound }
    }
}

impl Iterator for Incident<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        let take_out = match (self.out.first(), self.inbound.first()) {
            (Some(o), Some(i)) => o <= i,
            (Some(_), None) => true,
            (None, _) => false,
        };
        let side = if take_out {
            &mut self.out
        } else {
            &mut self.inbound
        };
        let (&first, rest) = side.split_first()?;
        *side = rest;
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(rows: u32, pairs: &[(u32, u32)]) -> Csr {
        Csr::from_pairs(rows, pairs.iter().copied()).expect("fits")
    }

    #[test]
    fn rows_keep_arrival_order_not_value_order() {
        let csr = build(3, &[(2, 9), (0, 5), (2, 1), (0, 4), (2, 7)]);
        assert_eq!(csr.row(0), [5, 4]);
        assert_eq!(csr.row(1), [] as [u32; 0]);
        assert_eq!(csr.row(2), [9, 1, 7]);
        assert_eq!((csr.rows(), csr.len()), (3, 5));
        assert!(!csr.is_empty());
    }

    #[test]
    fn empty_and_zero_row_adjacencies_are_well_formed() {
        let none = build(0, &[]);
        assert_eq!((none.rows(), none.len()), (0, 0));
        assert!(none.is_empty());
        assert_eq!(none, Csr::default());
        let bare = build(2, &[]);
        assert_eq!(
            (bare.rows(), bare.row(0), bare.row(1)),
            (2, &[][..], &[][..])
        );
    }

    #[test]
    fn a_repeated_pair_is_kept_twice() {
        let csr = build(1, &[(0, 3), (0, 3)]);
        assert_eq!(csr.row(0), [3, 3]);
    }

    #[test]
    fn byte_len_counts_offsets_and_values() {
        assert_eq!(build(3, &[(1, 1), (1, 2)]).byte_len(), 4 * 4 + 2 * 4);
    }

    #[test]
    fn incident_merges_two_ascending_rows_keeping_ties_twice() {
        let merged: Vec<u32> = Incident::merge(&[1, 4, 6], &[0, 4, 5, 9]).collect();
        assert_eq!(merged, [0, 1, 4, 4, 5, 6, 9]);
        assert_eq!(Incident::merge(&[], &[2]).collect::<Vec<_>>(), [2]);
    }

    #[test]
    #[should_panic(expected = "row 2 of 2")]
    fn a_row_out_of_range_panics() {
        build(2, &[(2, 0)]);
    }

    #[test]
    fn a_row_count_past_the_u32_index_space_is_the_capacity_error() {
        let refused = Csr::from_pairs(u32::MAX, std::iter::empty());
        assert_eq!(refused, Err(CapacityError { what: "adjacency" }));
    }

    /// Yields `(0, 7)` once across all its clones: the counting walk sees one pair, the
    /// placing walk none.
    #[derive(Clone)]
    struct Once(std::rc::Rc<std::cell::Cell<bool>>);

    impl Iterator for Once {
        type Item = (u32, u32);

        fn next(&mut self) -> Option<(u32, u32)> {
            (!self.0.replace(true)).then_some((0, 7))
        }
    }

    #[test]
    #[should_panic(expected = "different sequence on the second pass")]
    fn a_second_pass_shorter_than_the_first_panics() {
        let _ = Csr::from_pairs(1, Once(std::rc::Rc::default()));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Incident::merge needs ascending rows")]
    fn merging_an_unsorted_row_panics_in_a_debug_build() {
        let _ = Incident::merge(&[3, 1], &[2]);
    }
}
