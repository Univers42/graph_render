//! The gradient of the Kamada-Kawai energy `E`, one vertex at a time, summed over the other
//! vertices in ascending index so the order is a function of the vertex order alone (D3).

use super::springs::Springs;
use super::{separation, squared};

/// Gradient of `E` on `m` alone, summed over the other vertices in ascending index (D3).
pub(super) fn full_gradient<const D: usize>(
    pos: &[[f64; D]],
    springs: &Springs,
    m: usize,
) -> [f64; D] {
    let mut g = [0.0; D];
    for i in (0..springs.n).filter(|&i| i != m) {
        let p = pull(pos, springs, m, i);
        for axis in 0..D {
            g[axis] += p[axis];
        }
    }
    g
}

/// Gradient contribution on `m` of its spring to `i`: `k * (delta - l * delta / |delta|)`.
/// A coincident pair contributes its pure stretch term only, the direction being undefined.
pub(super) fn pull<const D: usize>(
    pos: &[[f64; D]],
    springs: &Springs,
    m: usize,
    i: usize,
) -> [f64; D] {
    let (k, l) = springs.spring(m, i);
    let delta = separation(&pos[m], &pos[i]);
    let r = libm::sqrt(squared(&delta));
    let shrink = if r > 0.0 { l / r } else { 0.0 };
    let mut p = [0.0; D];
    for (axis, d) in delta.iter().enumerate() {
        p[axis] = k * (d - shrink * d);
    }
    p
}
