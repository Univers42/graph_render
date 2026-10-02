//! Hop distances between every pair and the two spring constants derived from them: the
//! energy model's inputs, and everything in it that is dimension-independent.

use super::SimpleGraph;

/// Hop distances and the two per-pair constants derived from them.
pub(super) struct Springs {
    pub(super) n: usize,
    dist: Vec<f64>,
    length_per_hop: f64,
    strength: f64,
}

impl Springs {
    /// `l_ij = sqrt(n) / d_max * d_ij` and `k_ij = K / d_ij^2`. An unreachable distance, and
    /// a zero one, both become `d_max`, so a disconnected graph's components are `d_max` apart.
    /// Ponytail: an edgeless graph has no finite `d_max` and igraph divides by zero there; this
    /// port takes every distance as 1 and draws a regular-polygon-like cloud instead.
    pub(super) fn new(graph: &SimpleGraph, n: usize, strength: f64) -> Self {
        let mut dist = all_pairs_hops(graph, n);
        let finite_max = dist
            .iter()
            .copied()
            .filter(|d| d.is_finite())
            .fold(0.0, f64::max);
        let d_max = if finite_max > 0.0 { finite_max } else { 1.0 };
        for d in dist.iter_mut().filter(|d| !d.is_finite() || **d == 0.0) {
            *d = d_max;
        }
        Springs {
            n,
            dist,
            length_per_hop: libm::sqrt(n as f64) / d_max,
            strength,
        }
    }

    /// Spring `(k, l)` between `i` and `j`.
    pub(super) fn spring(&self, i: usize, j: usize) -> (f64, f64) {
        let d = self.dist[i * self.n + j];
        (self.strength / (d * d), self.length_per_hop * d)
    }
}

/// Breadth-first hop counts from every vertex; unreachable is infinity. Ascending source,
/// ascending queue order, so the matrix is a function of the edge order alone (D3).
fn all_pairs_hops(graph: &SimpleGraph, n: usize) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; n * n];
    let mut queue = Vec::with_capacity(n);
    for s in 0..n {
        queue.clear();
        queue.push(s);
        dist[s * n + s] = 0.0;
        let mut head = 0;
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            for &e in graph.rows.row(v as u32) {
                let e = e as usize;
                let u = if graph.lo[e] as usize == v {
                    graph.hi[e]
                } else {
                    graph.lo[e]
                } as usize;
                if dist[s * n + u].is_infinite() {
                    dist[s * n + u] = dist[s * n + v] + 1.0;
                    queue.push(u);
                }
            }
        }
    }
    dist
}
