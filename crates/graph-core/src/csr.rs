//! Compressed sparse rows: `offsets` (rows + 1) and `values`, both `u32` (D6).
//!
//! Built in O(rows + pairs) by a counting sort that is **stable**: within a row, values
//! keep the order the pairs arrived in. The topology feeds pairs in edge order, so every
//! row lists its edges in the oracle's `Map` insertion order (`model.ts:41-42`). A CSR
//! sorted by value would be more cache-friendly and would break the differential.

use crate::arena::CapacityError;

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
    /// both times. A row index `>= rows` is a caller bug and panics.
    pub fn from_pairs<I>(rows: u32, pairs: I) -> Result<Self, CapacityError>
    where
        I: Iterator<Item = (u32, u32)> + Clone,
    {
        let overflow = CapacityError { what: "adjacency" };
        let mut offsets = vec![0u32; rows as usize + 1];
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
        let mut values = vec![0u32; offsets[rows as usize] as usize];
        for (row, value) in pairs {
            let at = &mut cursor[row as usize];
            values[*at as usize] = value;
            *at += 1;
        }
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
    pub fn merge(out: &'a [u32], inbound: &'a [u32]) -> Self {
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
}
