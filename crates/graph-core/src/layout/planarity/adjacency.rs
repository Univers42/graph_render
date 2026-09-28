//! The simple graph the LR test and the embedding builder both walk: multi-edges and
//! self-loops removed (SciGraphs' own `simple` reduction,
//! `circle_packing.py:_planar_triangulation`), each row sorted ascending by the other
//! endpoint — a dense-index total order, never hash order, and the order a slot's
//! position is found in by binary search rather than a lookup table.

/// `n` nodes, each row (`row(v)`) the other endpoint of every incident edge, ascending.
/// A **slot** is a flat index into the shared `neighbours` array: `row(v)`'s slots are
/// `start(v)..start(v) + degree(v)`, contiguous and sorted, so [`Self::slot`] can binary
/// search them. Two slots always exist per edge, one per endpoint's row, and neither is
/// a self-loop.
#[derive(Debug, Clone)]
pub(super) struct Adjacency {
    offsets: Vec<u32>,
    neighbours: Vec<u32>,
}

impl Adjacency {
    /// Builds the simple graph on `n` nodes from raw `edges`: an endpoint at or past `n`
    /// drops the edge, a self-loop drops it, and a repeated pair collapses to one.
    pub(super) fn simple(n: u32, edges: &[(u32, u32)]) -> Self {
        let mut pairs: Vec<(u32, u32)> = edges
            .iter()
            .copied()
            .filter(|&(a, b)| a < n && b < n && a != b)
            .map(|(a, b)| if a < b { (a, b) } else { (b, a) })
            .collect();
        pairs.sort_unstable();
        pairs.dedup();
        Self::from_sorted_pairs(n, &pairs)
    }

    /// Counts each endpoint's degree, then scatters both directions of every pair; a
    /// pair list sorted ascending by `(a, b)` leaves every row sorted ascending too (a
    /// row's low neighbours arrive as `a`, its high neighbours as `b`, each block
    /// already ascending, one strictly below the other).
    fn from_sorted_pairs(n: u32, pairs: &[(u32, u32)]) -> Self {
        let mut offsets = vec![0u32; n as usize + 1];
        for &(a, b) in pairs {
            offsets[a as usize + 1] += 1;
            offsets[b as usize + 1] += 1;
        }
        for i in 1..offsets.len() {
            offsets[i] += offsets[i - 1];
        }
        let mut cursor = offsets.clone();
        let mut neighbours = vec![0u32; offsets[n as usize] as usize];
        let mut place = |row: u32, value: u32| {
            let at = &mut cursor[row as usize];
            neighbours[*at as usize] = value;
            *at += 1;
        };
        for &(a, b) in pairs {
            place(a, b);
            place(b, a);
        }
        Self {
            offsets,
            neighbours,
        }
    }

    /// Nodes, real ones only (no virtual root here: planarity is edge combinatorics).
    pub(super) fn node_count(&self) -> u32 {
        self.offsets.len() as u32 - 1
    }

    /// Simple-graph edges, `neighbours.len() / 2`.
    pub(super) fn edge_count(&self) -> u32 {
        self.neighbours.len() as u32 / 2
    }

    /// Every slot across every row.
    pub(super) fn total_slots(&self) -> u32 {
        self.neighbours.len() as u32
    }

    /// `v`'s neighbours, ascending.
    pub(super) fn row(&self, v: u32) -> &[u32] {
        let (a, b) = (self.offsets[v as usize], self.offsets[v as usize + 1]);
        &self.neighbours[a as usize..b as usize]
    }

    /// `v`'s first slot; meaningless when `v` has no neighbour.
    pub(super) fn start(&self, v: u32) -> u32 {
        self.offsets[v as usize]
    }

    /// `v`'s degree.
    pub(super) fn degree(&self, v: u32) -> u32 {
        self.offsets[v as usize + 1] - self.offsets[v as usize]
    }

    /// The slot for edge `(v, x)`. Panics if `x` is not one of `v`'s neighbours: every
    /// caller here only ever asks about an edge it already knows exists.
    pub(super) fn slot(&self, v: u32, x: u32) -> u32 {
        let found = self
            .row(v)
            .binary_search(&x)
            .expect("x is a neighbour of v");
        self.start(v) + found as u32
    }

    /// The neighbour a slot stands for.
    pub(super) fn slot_neighbour(&self, slot: u32) -> u32 {
        self.neighbours[slot as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_ends_up_sorted_ascending_regardless_of_input_order() {
        let a = Adjacency::simple(4, &[(3, 0), (1, 3), (0, 1), (2, 1)]);
        assert_eq!(a.row(0), [1, 3]);
        assert_eq!(a.row(1), [0, 2, 3]);
        assert_eq!(a.row(2), [1]);
        assert_eq!(a.row(3), [0, 1]);
        assert_eq!((a.node_count(), a.edge_count(), a.total_slots()), (4, 4, 8));
    }

    #[test]
    fn self_loops_and_repeats_and_out_of_range_endpoints_are_dropped() {
        let a = Adjacency::simple(2, &[(0, 0), (0, 1), (1, 0), (0, 1), (0, 5), (5, 0)]);
        assert_eq!((a.row(0), a.row(1)), (&[1][..], &[0][..]));
        assert_eq!(a.edge_count(), 1);
    }

    #[test]
    fn slot_and_slot_neighbour_round_trip() {
        let a = Adjacency::simple(3, &[(0, 1), (1, 2), (0, 2)]);
        for v in 0..3 {
            for &x in a.row(v) {
                assert_eq!(a.slot_neighbour(a.slot(v, x)), x);
            }
        }
        // Two distinct slots per edge, one per endpoint.
        assert_ne!(a.slot(0, 1), a.slot(1, 0));
    }

    #[test]
    fn an_edgeless_graph_has_empty_rows_and_no_slots() {
        let a = Adjacency::simple(3, &[]);
        assert_eq!((a.row(0), a.row(1), a.row(2)), (&[][..], &[][..], &[][..]));
        assert_eq!(a.total_slots(), 0);
    }
}
