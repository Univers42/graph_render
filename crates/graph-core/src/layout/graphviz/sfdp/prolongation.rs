//! Laying a coarse solution back down: the `P` multiply, the neighbour interpolation, the
//! jitter that breaks a matched pair apart.
//!
//! Reference: `prolongate` and `interpolate_coord` at `lib/sfdpgen/spring_electrical.c:837-852`
//! and `:814-835`, called from the multilevel driver at `:1155`. Three steps, in that order:
//!
//! 1. **`P` multiply.** `SparseMatrix_multiply_dense(P, xc, xf, dim)` — every fine node takes
//!    its coarse node's position. `P`'s entries are all 1 and each fine node appears in exactly
//!    one row, so the product is a gather from `xc` through [`Level::pair`].
//! 2. **`interpolate_coord`.** Every node is pulled `alpha = 0.5` of the way toward the mean of
//!    its neighbours, `beta = (1-alpha)/nz`, self edges skipped, **in place and in CSR row
//!    order** — so a node reads neighbours a lower index has already moved (Gauss-Seidel).
//! 3. **Jitter.** Every member of an `R` row *after the first* takes `delta·(drand() - 0.5)`
//!    per coordinate, `delta = K·0.001` with the `K` the driver holds **before** the 0.75 decay
//!    (`:1155` runs before `:1159`).
//!
//! Steps 1 and 2 are what separate a matched pair at all: without them both members land on one
//! point and the spring between them is of length zero forever. The jitter is what keeps the
//! pair from being *bit*-coincident.
//!
//! Determinism (D2): the CSR rows and the cluster walk are both in dense index order, and the
//! jitter draws come from the same glibc `drand()` stream the random start consumed, in level
//! order — not from a counter-based generator, so two nodes of one cluster get two draws of
//! that stream in a fixed order.
//!
//! Ponytail: **the order within one `R` row is ascending fine index, not the reference's.**
//! The reference appends a cluster's members in the order its matcher emitted them, which comes
//! out of `gv_permutation` (`Multilevel.c:114`), and no port that does not draw that permutation
//! can match it. Ascending index is the same set of draws on a different node. Failing input:
//! every graph, in which fine node of a cluster gets which draw. Direction: the separation is the
//! same magnitude in a different direction. Escape hatch: [`Level::pair`]'s producer,
//! `matching::pass`, which is where a port would have to record the cluster order.

use super::multilevel::Level;
use super::start::Glibc;
use crate::csr::Csr;

/// How hard the reference pulls a node toward its neighbours' mean
/// (`interpolate_coord`, `spring_electrical.c:817`).
const ALPHA: f64 = 0.5;

/// `delta = K * 0.001` (`spring_electrical.c:1155`): the jitter is a thousandth of the ideal
/// edge length, and the reference's is 1000 times smaller than the coarse spacing.
const DELTA_SCALE: f64 = 0.001;

/// Everything one prolongation needs beyond the coarse positions: the level to lay down, the
/// fine graph to interpolate over, how many fine nodes there are, and the jitter scale.
pub(super) struct Lay<'a> {
    pub(super) level: &'a Level,
    pub(super) edges: &'a [(u32, u32)],
    pub(super) count: u32,
    pub(super) delta: f64,
}

/// `prolongate`: the coarse solution on `count` fine nodes, as `(x, y)`.
///
/// `rng` is the generator the layout's random start was drawn from, still positioned where the
/// coarsest level left it: the reference draws the jitter from the same `srand`-seeded stream.
pub(super) fn prolongate(
    coarse_x: &[f64],
    coarse_y: &[f64],
    lay: &Lay<'_>,
    rng: &mut Glibc,
) -> (Vec<f64>, Vec<f64>) {
    let (mut x, mut y) = multiply_p(coarse_x, coarse_y, lay);
    interpolate(&neighbours(lay), &mut x, &mut y);
    jitter(&mut x, &mut y, lay, rng);
    (x, y)
}

/// `SparseMatrix_multiply_dense(P, xc, xf, dim)`: fine node `i` takes `xc[pair[i]]`.
fn multiply_p(coarse_x: &[f64], coarse_y: &[f64], lay: &Lay<'_>) -> (Vec<f64>, Vec<f64>) {
    let count = lay.count as usize;
    let mut x = vec![0.0; count];
    let mut y = vec![0.0; count];
    for i in 0..count {
        let c = lay.level.pair[i] as usize;
        x[i] = coarse_x.get(c).copied().unwrap_or(0.0);
        y[i] = coarse_y.get(c).copied().unwrap_or(0.0);
    }
    (x, y)
}

/// `interpolate_coord(dim, A, x)` over the fine graph, in place and in row order.
///
/// **Gauss-Seidel, not Jacobi.** The reference writes `x[i]` inside the same loop that reads
/// `x[ja[j]]`, so a node whose neighbour has a lower index sees the moved neighbour. Collecting
/// the sums into a second array first would be a different algorithm, and on a path it gives a
/// different drawing.
fn interpolate(rows: &Csr, x: &mut [f64], y: &mut [f64]) {
    for (i, slot) in x.iter_mut().enumerate() {
        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut nz = 0u32;
        for &j in rows.row(i as u32) {
            sum_x += x[j as usize];
            sum_y += y[j as usize];
            nz += 1;
        }
        if nz == 0 {
            continue;
        }
        let beta = (1.0 - ALPHA) / f64::from(nz);
        *slot = ALPHA * *slot + beta * sum_x;
        y[i] = ALPHA * y[i] + beta * sum_y;
    }
}

/// Every member of an `R` row after the first takes `delta·(drand() - 0.5)` per coordinate.
///
/// Walking the fine nodes in ascending index and skipping each coarse node's first member *is*
/// "every `R`-row member after the first" in ascending order: the first member of a cluster is
/// the first index carrying that coarse id.
fn jitter(x: &mut [f64], y: &mut [f64], lay: &Lay<'_>, rng: &mut Glibc) {
    let mut first = vec![true; lay.level.coarse as usize];
    for i in 0..lay.count as usize {
        let c = lay.level.pair[i] as usize;
        if std::mem::replace(&mut first[c], false) {
            continue;
        }
        x[i] += lay.delta * (rng.unit() - 0.5);
        y[i] += lay.delta * (rng.unit() - 0.5);
    }
}

/// The fine graph as neighbour rows, self edges dropped (`if (ja[j] == i) continue`).
fn neighbours(lay: &Lay<'_>) -> Csr {
    let pairs = lay
        .edges
        .iter()
        .filter(|&&(a, b)| a != b)
        .flat_map(|&(a, b)| [(a, b), (b, a)]);
    Csr::from_pairs(lay.count, pairs).expect("an edge count fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level_of(pair: Vec<u32>, coarse: u32) -> Level {
        Level { pair, coarse }
    }

    /// The `P` multiply alone: every fine node lands on its coarse node's position.
    #[test]
    fn the_multiply_gathers_through_pair() {
        let level = level_of(vec![1, 0, 1], 2);
        let lay = Lay {
            level: &level,
            edges: &[],
            count: 3,
            delta: 0.0,
        };
        let (x, _) = prolongate(&[10.0, 20.0], &[1.0, 2.0], &lay, &mut Glibc::seeded(1));
        assert_eq!(x, vec![20.0, 10.0, 20.0], "the multiply is not a gather");
    }

    /// An edgeless level: `nz = 0` everywhere, so `interpolate_coord` must leave every position
    /// alone rather than divide by a count of zero.
    #[test]
    fn an_isolated_level_is_left_alone() {
        let level = level_of(vec![0, 1], 2);
        let lay = Lay {
            level: &level,
            edges: &[],
            count: 2,
            delta: 0.0,
        };
        let (x, y) = prolongate(&[3.0, 4.0], &[5.0, 6.0], &lay, &mut Glibc::seeded(1));
        assert_eq!((x, y), (vec![3.0, 4.0], vec![5.0, 6.0]));
    }

    /// A node with exactly one neighbour is pulled the whole half-way: `beta = 0.5/1`, so the
    /// result is the mean of the two, which is the boundary case of the `(1-alpha)/nz` form.
    #[test]
    fn a_leaf_is_pulled_half_way_to_its_neighbour() {
        let level = level_of(vec![0, 0], 1);
        let lay = Lay {
            level: &level,
            edges: &[(0u32, 1u32)],
            count: 2,
            delta: 0.0,
        };
        let (x, _) = prolongate(&[0.0], &[10.0], &lay, &mut Glibc::seeded(1));
        assert!((x[0] - 5.0).abs() < 1e-12, "node 0 at {} want 5", x[0]);
        assert!((x[1] - 5.0).abs() < 1e-12, "node 1 at {} want 5", x[1]);
    }

    /// `interpolate_coord` is Gauss-Seidel: node 0 reads node 2 *after* node 1 moved it.
    #[test]
    fn interpolation_is_in_row_order_not_jacobi() {
        let level = level_of(vec![0, 1, 2], 3);
        let lay = Lay {
            level: &level,
            edges: &[(0u32, 1u32), (1, 2)],
            count: 3,
            delta: 0.0,
        };
        let (x, _) = prolongate(&[0.0, 10.0, 20.0], &[0.0; 3], &lay, &mut Glibc::seeded(1));
        // Row order: node 0 -> mean(0,10) = 5; node 1 -> mean(5,20) = 12.5;
        // node 2 -> mean(12.5, 20) = 16.25. Jacobi would give 15 for the last.
        assert!((x[0] - 5.0).abs() < 1e-12, "node 0 at {}", x[0]);
        assert!((x[1] - 12.5).abs() < 1e-12, "node 1 at {}", x[1]);
        assert!((x[2] - 16.25).abs() < 1e-12, "node 2 at {}", x[2]);
    }

    /// A coarse node's **first** member is never jittered; every other member is, and by at most
    /// half of `delta`. This is the property that keeps a matched pair from being coincident
    /// while leaving the drawing undisplaced.
    #[test]
    fn only_the_second_member_of_a_pair_is_jittered() {
        let level = level_of(vec![0, 0, 1, 1], 2);
        let lay = Lay {
            level: &level,
            edges: &[],
            count: 4,
            delta: 0.1,
        };
        let (x, y) = prolongate(&[10.0, 20.0], &[30.0, 40.0], &lay, &mut Glibc::seeded(1));
        assert_eq!((x[0], y[0]), (10.0, 30.0), "the first member moved");
        assert_eq!((x[2], y[2]), (20.0, 40.0), "the first member moved");
        for (i, (base_x, base_y)) in [(10.0, 30.0), (20.0, 40.0)].into_iter().enumerate() {
            let at = 1 + 2 * i;
            assert!(
                (x[at] - base_x).abs() <= 0.05 && (y[at] - base_y).abs() <= 0.05,
                "node {at} at ({}, {}), want within 0.05 of ({base_x}, {base_y})",
                x[at],
                y[at]
            );
            assert!(
                x[at] != base_x || y[at] != base_y,
                "node {at} was not jittered at all"
            );
        }
    }

    /// The jitter draws from the generator the caller hands in, so it continues that stream
    /// rather than restarting one: two calls with the same generator state agree, and one call
    /// moves the state.
    #[test]
    fn the_jitter_continues_the_stream_it_is_given() {
        let level = level_of(vec![0, 0], 1);
        let lay = Lay {
            level: &level,
            edges: &[],
            count: 2,
            delta: 0.1,
        };
        let mut a = Glibc::seeded(1);
        let mut b = Glibc::seeded(1);
        let first = prolongate(&[10.0], &[30.0], &lay, &mut a);
        let second = prolongate(&[10.0], &[30.0], &lay, &mut b);
        assert_eq!(first, second);
        let after = prolongate(&[10.0], &[30.0], &lay, &mut b);
        assert_ne!(first, after, "the stream did not advance");
    }
}