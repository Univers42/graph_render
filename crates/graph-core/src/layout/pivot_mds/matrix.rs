//! The dense steps between the hop-distance matrix and the eigensolve: double-centering,
//! the Gram matrix, and the projection back to points. Every sum adds its terms in the
//! order, and from the start value, of the `f64::sum` it replaced, so the bytes did not
//! move when the loops were turned to read `dist` row by row.

use crate::linalg::EigBlock;

/// What `Iterator::sum::<f64>()` starts from (`-0.0` in current Rust), so a sum accumulated
/// in place keeps the iterator sum's bytes, `-0.0` totals included.
pub(super) const SUM_ZERO: f64 = -0.0;

/// Rows of `dist` folded into the Gram matrix per pass over it.
const GRAM_BLOCK: usize = 4;

/// Squares `dist` in place, then double-centers it (`_pivot_mds_coordinates:187-194`),
/// sequential sums throughout (D3).
pub(super) fn double_center(dist: &mut [f64], n: usize, k: usize) {
    for v in dist.iter_mut() {
        *v *= *v;
    }
    let mut col_mean = vec![SUM_ZERO; k];
    for row in dist.chunks_exact(k) {
        for (mean, &v) in col_mean.iter_mut().zip(row) {
            *mean += v;
        }
    }
    for mean in &mut col_mean {
        *mean /= n as f64;
    }
    let grand_mean = col_mean.iter().sum::<f64>() / k as f64;
    for row in dist.chunks_exact_mut(k) {
        let row_mean = row.iter().sum::<f64>() / k as f64;
        for (v, &mean) in row.iter_mut().zip(&col_mean) {
            *v = (*v - mean - row_mean + grand_mean) * -0.5;
        }
    }
}

/// `distᵀ dist`, the `k x k` Gram matrix Pivot MDS's eigensolve runs on.
///
/// Row by row, so `dist` is read once and in order: a column-pair dot product strides by
/// `k` and took 1e9 cache misses at 100 000 nodes. [`GRAM_BLOCK`] rows go into each cell
/// per load and store, still one addition at a time in ascending row order from
/// `f64::sum`'s own start, so the bytes are the dot product's
/// (`gram_is_the_column_dot_product_bit_for_bit`). Only the upper triangle is summed: the
/// lower one is its mirror, as `a * b == b * a` exactly in IEEE 754.
pub(super) fn gram(dist: &[f64], k: usize) -> Vec<f64> {
    let mut g = vec![SUM_ZERO; k * k];
    let mut blocks = dist.chunks_exact(GRAM_BLOCK * k);
    for block in &mut blocks {
        add_block(&mut g, block, k);
    }
    for row in blocks.remainder().chunks_exact(k) {
        for (p, &a) in row.iter().enumerate() {
            for (cell, &b) in g[p * k + p..(p + 1) * k].iter_mut().zip(&row[p..]) {
                *cell += a * b;
            }
        }
    }
    for p in 0..k {
        for q in 0..p {
            g[p * k + q] = g[q * k + p];
        }
    }
    g
}

/// Adds [`GRAM_BLOCK`] consecutive rows' products to the upper triangle of `g`.
fn add_block(g: &mut [f64], block: &[f64], k: usize) {
    let (r0, rest) = block.split_at(k);
    let (r1, rest) = rest.split_at(k);
    let (r2, r3) = rest.split_at(k);
    for p in 0..k {
        let (a0, a1, a2, a3) = (r0[p], r1[p], r2[p], r3[p]);
        let cells = &mut g[p * k + p..(p + 1) * k];
        let (b0, b1, b2, b3) = (&r0[p..], &r1[p..], &r2[p..], &r3[p..]);
        for (q, cell) in cells.iter_mut().enumerate() {
            *cell = *cell + a0 * b0[q] + a1 * b1[q] + a2 * b2[q] + a3 * b3[q];
        }
    }
}

/// `dist @ vectors`: the `k`-dimensional eigenvectors projected back to `n_c` points
/// (`_pivot_mds_coordinates:198`) — sign-pinned on *this* result, not on `top` itself.
pub(super) fn project(dist: &[f64], n: usize, k: usize, top: &EigBlock) -> EigBlock {
    let dims_eff = top.k;
    let mut vectors = vec![0.0; n * dims_eff];
    for d in 0..dims_eff {
        let v = top.column(d);
        for (out, row) in vectors[d * n..(d + 1) * n].iter_mut().zip(dist.chunks_exact(k)) {
            *out = row.iter().zip(v).map(|(&a, &b)| a * b).sum();
        }
    }
    EigBlock {
        values: top.values.clone(),
        vectors,
        n,
        k: dims_eff,
    }
}
