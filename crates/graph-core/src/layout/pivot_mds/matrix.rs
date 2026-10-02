//! The dense steps between the hop counts and the eigensolve: double-centering, the Gram
//! matrix, and the projection back to points. Every sum adds its terms in the order, and
//! from the start value, of the `f64::sum` it replaced, so the bytes did not move when the
//! matrix was stored as counts and read a chunk of rows at a time.

use crate::linalg::EigBlock;

/// What `Iterator::sum::<f64>()` starts from (`-0.0` in current Rust), so a sum accumulated
/// in place keeps the iterator sum's bytes, `-0.0` totals included.
pub(super) const SUM_ZERO: f64 = -0.0;

/// Rows of the centered matrix folded into the Gram matrix per pass over it.
const GRAM_BLOCK: usize = 4;

/// Centered rows rebuilt per chunk: 512 rows of 100 pivots are 400 KB of `f64`, an L2's
/// worth, and every pivot column is read 2 KB at a time.
pub(super) const CHUNK_ROWS: usize = 512;

/// The squared hop counts, double-centered (`_pivot_mds_coordinates:187-194`), kept as the
/// counts themselves, one pivot's column after another, plus the means they are centered by.
///
/// The `f64` matrix this replaces was 800 MB at 1 000 000 nodes and 100 pivots, and a
/// pivot's walk wrote it one row stride apart; the counts are 400 MB and each walk writes
/// its column in order. A centered entry is rebuilt from its count by the same expression,
/// so it has the same bytes.
pub(super) struct Centered {
    hops: Vec<u32>,
    n: usize,
    k: usize,
    col_mean: Vec<f64>,
    row_mean: Vec<f64>,
    grand_mean: f64,
}

fn square(hops: u32) -> f64 {
    let d = f64::from(hops);
    d * d
}

impl Centered {
    /// Centers `hops`, column `j` being `hops[j * n..(j + 1) * n]`. Each column mean sums down
    /// its column and each row mean across its row, in order from [`SUM_ZERO`] (D3).
    pub(super) fn new(hops: Vec<u32>, n: usize, k: usize) -> Self {
        let mut col_mean = Vec::with_capacity(k);
        let mut row_mean = vec![SUM_ZERO; n];
        for column in hops.chunks_exact(n) {
            let mut sum = SUM_ZERO;
            for (row, &h) in row_mean.iter_mut().zip(column) {
                let v = square(h);
                sum += v;
                *row += v;
            }
            col_mean.push(sum / n as f64);
        }
        for row in &mut row_mean {
            *row /= k as f64;
        }
        let grand_mean = col_mean.iter().sum::<f64>() / k as f64;
        Self {
            hops,
            n,
            k,
            col_mean,
            row_mean,
            grand_mean,
        }
    }

    /// Centered rows `start..start + rows.len() / k`, row-major, into `rows`.
    fn rows_into(&self, start: usize, rows: &mut [f64]) {
        let (k, count) = (self.k, rows.len() / self.k);
        let row_mean = &self.row_mean[start..start + count];
        for (j, column) in self.hops.chunks_exact(self.n).enumerate() {
            let mean = self.col_mean[j];
            for (r, (&h, &row)) in column[start..start + count]
                .iter()
                .zip(row_mean)
                .enumerate()
            {
                rows[r * k + j] = (square(h) - mean - row + self.grand_mean) * -0.5;
            }
        }
    }

    /// Calls `visit(start, rows)` on consecutive chunks of centered rows, in row order.
    pub(super) fn for_each_chunk(&self, mut visit: impl FnMut(usize, &[f64])) {
        let mut scratch = vec![0.0; CHUNK_ROWS.min(self.n) * self.k];
        for start in (0..self.n).step_by(CHUNK_ROWS) {
            let rows = &mut scratch[..CHUNK_ROWS.min(self.n - start) * self.k];
            self.rows_into(start, rows);
            visit(start, rows);
        }
    }
}

/// `Cᵀ C`, the `k x k` Gram matrix Pivot MDS's eigensolve runs on.
///
/// Row by row, so `C` is read once and in order: a column-pair dot product strides by `k` and
/// took 1e9 cache misses at 100 000 nodes. Only the upper triangle is summed: the lower one is
/// its mirror, as `a * b == b * a` exactly in IEEE 754.
pub(super) fn gram(centered: &Centered) -> Vec<f64> {
    let k = centered.k;
    let mut g = vec![SUM_ZERO; k * k];
    centered.for_each_chunk(|_, rows| add_rows(&mut g, rows, k));
    mirror(&mut g, k);
    g
}

/// Adds every row's products to the upper triangle of `g`. [`GRAM_BLOCK`] rows go into each
/// cell per load and store, still one addition at a time in ascending row order, so the bytes
/// are the dot product's (`gram_is_the_column_dot_product_bit_for_bit`).
pub(super) fn add_rows(g: &mut [f64], rows: &[f64], k: usize) {
    let mut blocks = rows.chunks_exact(GRAM_BLOCK * k);
    for block in &mut blocks {
        add_block(g, block, k);
    }
    for row in blocks.remainder().chunks_exact(k) {
        for (p, &a) in row.iter().enumerate() {
            for (cell, &b) in g[p * k + p..(p + 1) * k].iter_mut().zip(&row[p..]) {
                *cell += a * b;
            }
        }
    }
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

/// Copies the upper triangle of `g` onto the lower.
pub(super) fn mirror(g: &mut [f64], k: usize) {
    for p in 0..k {
        for q in 0..p {
            g[p * k + q] = g[q * k + p];
        }
    }
}

/// `C @ vectors`: the `k`-dimensional eigenvectors projected back to `n_c` points
/// (`_pivot_mds_coordinates:198`) — sign-pinned on *this* result, not on `top` itself.
pub(super) fn project(centered: &Centered, top: &EigBlock) -> EigBlock {
    let (n, k, dims_eff) = (centered.n, centered.k, top.k);
    let mut vectors = vec![0.0; n * dims_eff];
    centered.for_each_chunk(|start, rows| {
        for d in 0..dims_eff {
            let (v, out) = (top.column(d), &mut vectors[d * n + start..]);
            for (out, row) in out.iter_mut().zip(rows.chunks_exact(k)) {
                *out = row.iter().zip(v).map(|(&a, &b)| a * b).sum();
            }
        }
    });
    EigBlock {
        values: top.values.clone(),
        vectors,
        n,
        k: dims_eff,
    }
}

#[cfg(test)]
mod tests;
