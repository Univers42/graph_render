//! Betweenness centrality by Brandes' algorithm (2001); petgraph has none. Split from
//! `centrality.rs` for the house's 300-line file limit.
//!
//! **Precondition: every edge weight is strictly positive**, stricter than the parent
//! module's non-negative one. A zero-weight edge relaxed after its head was settled
//! drops a predecessor, and an undirected zero-weight edge is a zero-length cycle with
//! no finite path count. igraph 0.11.9 refuses the same input
//! (`src/centrality/betweenness.c:436-437`); this checks it with a `debug_assert`.
//!
//! Path counts are [`PathCount`]s, not bare `f64`: a layered graph of about 2,050 nodes
//! already has 2^1024 shortest paths between two of its nodes, past `f64::MAX`, and
//! `inf / inf` would write NaN (D9).

use crate::csr_petgraph::{CsrDigraph, NodeIx};
use crate::index::Topology;
use petgraph::visit::{EdgeRef as _, IntoEdges as _};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Betweenness centrality (Brandes 2001): for every ordered pair `(s, t)`, the share of
/// shortest `s`→`t` paths through `v`, summed over every pair. O(n (n + m log n)) exact:
/// per source, Θ(n) fresh buffers plus one Dijkstra. Unnormalised, and an undirected
/// edge counts both ways: networkx 3.6's `normalized=False` on the symmetrised digraph,
/// twice its undirected value (path a–b–c: b is 2, not 1). Not tried past its declared
/// ceiling (`docs/measurements/phase07-analysis.md`). Never silently sampled: a sampled
/// variant, if it ever ships, is a separately named capability (step 4, stop-and-ask).
pub fn betweenness(topology: &Topology) -> Vec<f32> {
    debug_assert!(
        topology.edges().strength.iter().all(|&w| w > 0.0),
        "betweenness requires strictly positive edge weights, stricter than the module's \
         non-negative precondition: a zero weight breaks Brandes' path counts (module doc), \
         and a graph that may carry a negative edge belongs to paths::bellman_ford instead"
    );
    let n = topology.node_count() as usize;
    let mut score = vec![0.0f64; n];
    for s in 0..n as u32 {
        accumulate_from(topology, s, &mut score);
    }
    score.into_iter().map(|v| v as f32).collect()
}

/// One source's contribution: Dijkstra that also counts shortest paths (`sigma`) and
/// keeps every tied predecessor, then Brandes' back-accumulation over the finish order.
fn accumulate_from(topology: &Topology, s: u32, score: &mut [f64]) {
    let n = topology.node_count() as usize;
    let dag = shortest_path_dag(topology, s, n);
    let mut delta = vec![0.0f64; n];
    for &w in dag.order.iter().rev() {
        for &v in &dag.preds[w as usize] {
            let share = dag.sigma[v as usize].ratio(dag.sigma[w as usize]);
            delta[v as usize] += share * (1.0 + delta[w as usize]);
        }
        if w != s {
            score[w as usize] += delta[w as usize];
        }
    }
}

/// Brandes' shortest-path DAG from one source: the settle order, each node's path
/// count, and every tied predecessor.
struct Dag {
    order: Vec<u32>,
    sigma: Vec<PathCount>,
    preds: Vec<Vec<u32>>,
}

/// Dijkstra from `s`, additionally tracking the path count and every tied predecessor
/// of each node — not exposed by petgraph's own `dijkstra`. Precondition (module doc):
/// strictly positive weights, so a settled node is never an equal-distance successor.
fn shortest_path_dag(topology: &Topology, s: u32, n: usize) -> Dag {
    let graph = CsrDigraph::new(topology);
    let mut dist = vec![f64::INFINITY; n];
    let mut dag = Dag {
        order: Vec::with_capacity(n),
        sigma: vec![PathCount::ZERO; n],
        preds: vec![Vec::new(); n],
    };
    let mut done = vec![false; n];
    (dist[s as usize], dag.sigma[s as usize]) = (0.0, PathCount::ONE);
    let mut heap = BinaryHeap::from([MinScore(0.0, s)]);
    while let Some(MinScore(d, v)) = heap.pop() {
        if done[v as usize] {
            continue;
        }
        done[v as usize] = true;
        dag.order.push(v);
        for edge in graph.edges(NodeIx(v)) {
            let w = edge.target().0 as usize;
            if done[w] {
                continue;
            }
            let candidate = d + edge.weight();
            match candidate.partial_cmp(&dist[w]) {
                Some(Ordering::Less) => {
                    (dist[w], dag.sigma[w]) = (candidate, dag.sigma[v as usize]);
                    dag.preds[w] = vec![v];
                    heap.push(MinScore(candidate, w as u32));
                }
                Some(Ordering::Equal) => {
                    let through_v = dag.sigma[v as usize];
                    dag.sigma[w].add(through_v);
                    dag.preds[w].push(v);
                }
                _ => {}
            }
        }
    }
    dag
}

/// A `(distance, node)` pair ordered so [`BinaryHeap`] (a max-heap) pops the smallest
/// distance first, ties broken by the smaller dense index (D5) — the same discipline
/// `paths::dijkstra_distances` inherits from petgraph's own heap.
struct MinScore(f64, u32);

impl PartialEq for MinScore {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for MinScore {}
impl PartialOrd for MinScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MinScore {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .partial_cmp(&self.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.1.cmp(&self.1))
    }
}

/// A shortest-path count, `mantissa * 2^exponent`. Every count below 2^512 keeps
/// `exponent == 0`, so on such a graph [`PathCount::add`] and [`PathCount::ratio`] are
/// the plain `f64` `+=` and `/` bit for bit (`scalbn(x, 0) == x`): the rescale changes
/// no output that was already finite.
#[derive(Clone, Copy, Debug)]
struct PathCount {
    mantissa: f64,
    exponent: i32,
}

/// 2^512: a sum of two mantissas at most this stays below 2^513, far from `f64::MAX`.
const RESCALE_ABOVE: f64 = 1.340_780_792_994_259_7e154;
const RESCALE_BY: i32 = 512;

impl PathCount {
    const ZERO: Self = Self {
        mantissa: 0.0,
        exponent: 0,
    };
    const ONE: Self = Self {
        mantissa: 1.0,
        exponent: 0,
    };

    /// `self += other`, aligned to the larger exponent. Shifting the smaller mantissa
    /// down by a power of two loses only bits the `f64` sum would round away anyway.
    fn add(&mut self, other: Self) {
        if other.exponent > self.exponent {
            self.mantissa = libm::scalbn(self.mantissa, self.exponent - other.exponent);
            self.exponent = other.exponent;
            self.mantissa += other.mantissa;
        } else {
            self.mantissa += libm::scalbn(other.mantissa, other.exponent - self.exponent);
        }
        if self.mantissa > RESCALE_ABOVE {
            self.mantissa = libm::scalbn(self.mantissa, -RESCALE_BY);
            self.exponent += RESCALE_BY;
        }
    }

    /// `self / other` as a plain `f64`; a share below 2^-1074 reads 0, as it would in `f64`.
    fn ratio(self, other: Self) -> f64 {
        libm::scalbn(
            self.mantissa / other.mantissa,
            self.exponent - other.exponent,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescale_threshold_is_two_to_the_512() {
        assert_eq!(RESCALE_ABOVE, libm::scalbn(1.0, RESCALE_BY));
    }

    #[test]
    fn below_the_threshold_a_path_count_is_plain_f64_bit_for_bit() {
        let (mut count, mut plain) = (PathCount::ONE, 1.0f64);
        for step in [3.0, 0.1, 1e100, 7.5e153] {
            count.add(PathCount {
                mantissa: step,
                exponent: 0,
            });
            plain += step;
            assert_eq!(count.exponent, 0);
            assert_eq!(count.mantissa.to_bits(), plain.to_bits());
            assert_eq!(
                PathCount::ONE.ratio(count).to_bits(),
                (1.0 / plain).to_bits()
            );
        }
    }

    #[test]
    fn past_f64_max_a_path_count_stays_finite_and_exact_in_powers_of_two() {
        let mut count = PathCount::ONE;
        for _ in 0..1_100 {
            count.add(count);
        }
        let half = PathCount {
            mantissa: count.mantissa,
            exponent: count.exponent - 1,
        };
        assert!(count.mantissa.is_finite());
        assert_eq!(half.ratio(count), 0.5);
        assert_eq!(
            PathCount::ONE.ratio(count),
            0.0,
            "2^-1100 underflows, as in f64"
        );
    }
}
