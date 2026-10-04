//! The all-pairs shortest-path distances, in the packed upper triangle the iteration reads.
//!
//! Reference: `bfs.c` (`bfs`) and `stress.c:696-712` (`compute_apsp_packed`).
//!
//! `d_ij` is a **hop count**, not a length: `makeGraphData` allocates no `ewgts` unless the
//! graph carries a `len` attribute or the mode needs a direction (`neatoinit.c:770-776`),
//! and the differential's DOT files set neither, so every edge weighs exactly 1
//! (`bfs.c:35-37`). The port takes no lengths at all, which is a narrowing of the reference
//! and a deliberate one: a weighted search is a different code path with its own
//! convergence, and nothing in the ledger row or the oracle reaches it.
//!
//! **The packing is load-bearing.** The reference stores only the upper triangle, row-major,
//! row `i` holding `n - i` entries starting at `i*n - i*(i-1)/2`; every later pass walks it
//! with that stride and nothing indexes it as a matrix. Re-deriving the stride in
//! `Packed::at` rather than storing an index per entry is what keeps that the only place
//! the layout depends on it.
//!
//! **The unreachable-node fixup is the one place this port does not reproduce a choice.**
//! `bfs` gives a node it never reached the distance `closestDist + 10`, where `closestDist`
//! is the distance of the last node its queue emptied on — and with more than one component
//! in the graph, *which* component drained last is decided by the order the queue happened
//! to interleave them. This port uses its own queue and applies the same arithmetic, so a
//! connected graph agrees exactly and a disconnected one lands the unreachable node a
//! different, still finite, distance off. The module doc records it as a Ponytail.

use crate::csr::Csr;

/// The distance `bfs` gives a node it never reached, beyond the last one it did reach
/// (`bfs.c:47-49`).
const UNREACHED_BUMP: i32 = 10;

/// A node's shortest-path distances to every node, in dense index order.
///
/// `DIST_UNREACHED` marks a node no edge reaches, before the bump is applied.
const DIST_UNREACHED: i32 = -1;

/// The upper triangle of the distance matrix, row-major: row `i` holds `n - i` entries.
///
/// Only the upper triangle is stored because that is all the reference stores
/// (`stress.c:700-711`), and because both later passes — the Laplacian assembly and the
/// packed matrix-vector product — walk it in that order. A full `n x n` matrix would be
/// twice the memory for the same arithmetic.
pub(super) struct Packed {
    values: Vec<f32>,
    /// The node count, kept so a test can index a single entry. Nothing in the layout reads
    /// it: every pass walks `values` sequentially, as the reference does.
    #[cfg_attr(not(test), allow(dead_code))]
    count: usize,
}

impl Packed {
    /// Every pair's hop distance, `n` breadth-first searches in ascending source order.
    pub(super) fn of_hops(neighbours: &Csr, count: usize) -> Self {
        let mut values = vec![0.0f32; packed_len(count)];
        let mut row = vec![0i32; count];
        let mut queue = vec![0u32; count];
        for source in 0..count {
            let furthest = bfs(neighbours, source as u32, &mut row, &mut queue);
            let base = row_start(source, count);
            for offset in 0..(count - source) {
                let target = source + offset;
                values[base + offset] = hop(row[target], furthest);
            }
        }
        Self { values, count }
    }

    /// The packed array, in the order every later pass reads it.
    pub(super) fn as_slice(&self) -> &[f32] {
        &self.values
    }

    /// Entry `(i, j)` for `i <= j`, and entry `(j, i)` by symmetry for `i > j` — the
    /// reference's matrix is symmetric and stores one triangle of it, so every read has to
    /// make that choice explicitly rather than indexing a square.
    ///
    /// Only the tests read a single entry: the layout itself walks the packed array
    /// sequentially, exactly as the reference does, and indexing it by pair would be a
    /// *different* access pattern over the same numbers — faster, and not the port.
    #[cfg(test)]
    fn at(&self, i: usize, j: usize) -> f32 {
        let (low, high) = if i <= j { (i, j) } else { (j, i) };
        self.values[row_start(low, self.count) + (high - low)]
    }
}

/// How many entries the upper triangle holds: `n * (n + 1) / 2`.
fn packed_len(count: usize) -> usize {
    count * (count + 1) / 2
}

/// Where row `row` of the upper triangle starts: `row * n - row * (row - 1) / 2`, the
/// stride `stress.c:944` and `conjgrad.c`'s packed products both walk by.
fn row_start(row: usize, count: usize) -> usize {
    // `row * n - row * (row - 1) / 2`, factored so that `row == 0` does not underflow on
    // `row - 1`. Both forms are the same integer; this one is safe at the boundary.
    row * (2 * count - row + 1) / 2
}

/// One `bfs` from `source`, into `row`, on `queue` as the queue itself.
///
/// Returns the distance of the **last node the queue emptied on**, which is what the
/// reference's `closestDist` holds when the fixup runs: for a connected graph that is the
/// source's eccentricity, and it does not depend on the queue's order. `queue` is reused
/// across searches so a thousand of them allocate nothing.
fn bfs(neighbours: &Csr, source: u32, row: &mut [i32], queue: &mut [u32]) -> i32 {
    row.iter_mut().for_each(|slot| *slot = DIST_UNREACHED);
    row[source as usize] = 0;
    let mut head = 0;
    let mut tail = 0;
    queue[tail] = source;
    tail += 1;
    let mut furthest = 0;
    while head < tail {
        let node = queue[head];
        head += 1;
        furthest = row[node as usize];
        for &other in neighbours.row(node) {
            if row[other as usize] < 0 {
                // Every edge weighs one (`bfs.c:35-37`), so the bump is the constant 1 and
                // the whole weighted branch of the reference is unreachable from here.
                row[other as usize] = furthest + 1;
                queue[tail] = other;
                tail += 1;
            }
        }
    }
    furthest
}

/// One entry of the distance row: the hop count, or the fixup for a node never reached.
fn hop(distance: i32, furthest: i32) -> f32 {
    if distance < 0 {
        (furthest + UNREACHED_BUMP) as f32
    } else {
        distance as f32
    }
}

#[cfg(test)]
mod tests {
    use super::{Packed, packed_len, row_start};
    use crate::csr::Csr;

    fn hops(count: u32, edges: &[(u32, u32)]) -> Packed {
        let pairs: Vec<(u32, u32)> = edges.iter().flat_map(|&(a, b)| [(a, b), (b, a)]).collect();
        let csr = Csr::from_pairs(count, pairs.into_iter()).expect("rows in range");
        Packed::of_hops(&csr, count as usize)
    }

    /// The triangle holds `n*(n+1)/2` entries, which is the arithmetic every later stride
    /// is derived from; a change here silently reinterprets the whole matrix.
    #[test]
    fn the_triangle_holds_half_the_square_plus_the_diagonal() {
        assert_eq!(packed_len(0), 0);
        assert_eq!(packed_len(1), 1);
        assert_eq!(packed_len(4), 10);
        assert_eq!(packed_len(9), 45);
    }

    /// The stride is the reference's, not a re-derivation: row 0 starts at 0 and holds `n`
    /// entries, row 1 at `n` and holds `n-1`, and row `n-1` at the last entry alone.
    #[test]
    fn each_row_starts_where_the_previous_one_ended() {
        let count = 6;
        let mut expected = 0;
        for row in 0..count {
            assert_eq!(row_start(row, count), expected, "row {row}");
            expected += count - row;
        }
        assert_eq!(expected, packed_len(count));
    }

    /// A 3-path: adjacent nodes are one hop apart and the ends two, which is the stress
    /// model's only input and the property the closed-case tests lean on.
    #[test]
    fn a_path_is_one_hop_along_and_two_across() {
        let got = hops(3, &[(0, 1), (1, 2)]);
        assert_eq!(got.at(0, 0), 0.0);
        assert_eq!(got.at(0, 1), 1.0);
        assert_eq!(got.at(0, 2), 2.0);
        assert_eq!(got.at(1, 2), 1.0);
        assert_eq!(got.as_slice().len(), packed_len(3));
    }

    /// Reading a pair by either orientation gives the same number, because the matrix is
    /// symmetric and only one triangle of it is stored.
    #[test]
    fn the_matrix_is_read_by_symmetry() {
        let got = Packed::of_hops(
            &Csr::from_pairs(3, [(0, 1), (1, 0), (1, 2), (2, 1)].into_iter())
                .expect("rows in range"),
            3,
        );
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(got.at(i, j), got.at(j, i), "({i},{j})");
            }
        }
        assert_eq!(got.at(0, 0), 0.0);
        assert_eq!(got.at(0, 2), 2.0);
    }

    /// A node no edge reaches is given the last reached distance plus ten, never infinity
    /// and never left at `-1` — the reference's fixup (`bfs.c:47-49`), and the reason a
    /// disconnected graph still gets a finite drawing.
    ///
    /// A 4-path plus an isolated node, because a two-node component and a cycle cannot show
    /// what the fixup actually keys on. `closestDist` is the distance of the **last node the
    /// queue emptied on**, which for a connected search is the source's eccentricity: 3 from
    /// either end of the path, 2 from either interior node. So `d(0,4)` and `d(3,4)` are 13
    /// while `d(1,4)` and `d(2,4)` are 12 — the same unreachable node, two different
    /// "ten past the end" distances, and the interior ones are the property of the graph
    /// while which queue drains last is not.
    #[test]
    fn an_unreached_node_is_bumped_beyond_the_component() {
        let csr = Csr::from_pairs(
            5,
            [(0, 1), (1, 0), (1, 2), (2, 1), (2, 3), (3, 2)].into_iter(),
        )
        .expect("rows in range");
        let got = Packed::of_hops(&csr, 5);
        assert_eq!(got.at(0, 4), 13.0);
        assert_eq!(got.at(1, 4), 12.0);
        assert_eq!(got.at(2, 4), 12.0);
        assert_eq!(got.at(3, 4), 13.0);
        // The path's own distances are untouched by the isolated node's presence.
        assert_eq!(got.at(0, 1), 1.0);
        assert_eq!(got.at(0, 3), 3.0);
    }
}
