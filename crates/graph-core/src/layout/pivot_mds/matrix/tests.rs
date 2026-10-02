//! The chunked, count-backed matrix against the textbook sums it replaced, bit for bit.

use super::*;

/// The next value of a fixed LCG.
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1);
    *state >> 11
}

/// `n x k` row-major values with exact zeros of both signs mixed in. Column 0 is positive
/// and the last column all `-0.0`, so their dot product is a sum of `-0.0`s: the one case
/// where a sum's start shows in its bytes.
fn signed_zero_matrix(n: usize, k: usize) -> Vec<f64> {
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut cells = Vec::with_capacity(n * k);
    for i in 0..n * k {
        let r = lcg(&mut state);
        let v = r as f64 / (1u64 << 53) as f64 - 0.5;
        cells.push(match (i % k, r % 7) {
            (0, _) => 1.0 + v,
            (j, _) if j == k - 1 => -0.0,
            (_, 0) => -0.0,
            (_, 1) => 0.0,
            _ => v * 1e3,
        });
    }
    cells
}

#[test]
fn gram_is_the_column_dot_product_bit_for_bit() {
    assert_eq!(
        SUM_ZERO.to_bits(),
        std::iter::empty::<f64>().sum::<f64>().to_bits()
    );
    let (n, k) = (257, 13);
    let dist = signed_zero_matrix(n, k);
    let mut g = vec![SUM_ZERO; k * k];
    // Five-row chunks, so every chunk ends on a remainder shorter than a block.
    for rows in dist.chunks(5 * k) {
        add_rows(&mut g, rows, k);
    }
    mirror(&mut g, k);
    let mut dot_products = Vec::with_capacity(k * k);
    for p in 0..k {
        for q in 0..k {
            let dot: f64 = (0..n).map(|i| dist[i * k + p] * dist[i * k + q]).sum();
            dot_products.push(dot.to_bits());
        }
    }
    assert_eq!(
        g.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
        dot_products
    );
}

#[test]
fn centered_rows_are_the_row_major_double_centering_bit_for_bit() {
    let (n, k) = (2 * CHUNK_ROWS + 37, 7);
    let mut state = 7_u64;
    let hops: Vec<u32> = (0..n * k).map(|_| (lcg(&mut state) % 40) as u32).collect();
    let squared = |i: usize, j: usize| f64::from(hops[j * n + i]) * f64::from(hops[j * n + i]);
    let col = |j: usize| (0..n).map(|i| squared(i, j)).sum::<f64>() / n as f64;
    let row = |i: usize| (0..k).map(|j| squared(i, j)).sum::<f64>() / k as f64;
    let grand = (0..k).map(col).sum::<f64>() / k as f64;
    let mut seen = 0;
    Centered::new(hops.clone(), n, k).for_each_chunk(|start, rows| {
        assert_eq!(start, seen);
        for (r, values) in rows.chunks_exact(k).enumerate() {
            let i = start + r;
            for (j, v) in values.iter().enumerate() {
                let expect = (squared(i, j) - col(j) - row(i) + grand) * -0.5;
                assert_eq!(v.to_bits(), expect.to_bits(), "({i}, {j})");
            }
        }
        seen += rows.len() / k;
    });
    assert_eq!(seen, n);
}
