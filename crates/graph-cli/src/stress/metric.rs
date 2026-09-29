//! The stress metric itself (`graph-cli stress`'s measuring half), kept apart from the
//! command in `../stress.rs` so neither file outgrows the house line limit.
//!
//! **The metric** is the Pearson correlation between graph (BFS hop) distance and
//! Euclidean (layout) distance, over the pairs `(pivot, v)` for `v` any node reachable
//! from `pivot`, across a fixed max-min (farthest-point) pivot set.
//!
//! **Why a correlation and not a distance.** A force layout has no correct answer to
//! compare against: d3-force, networkx's FA2 and this port are three defensible
//! implementations of three different algorithms, and even two runs of the *same*
//! implementation at a different summation order settle somewhere else. What is
//! comparable is whether the picture *respects the graph's own distances*, and that is
//! exactly what a hop-to-euclidean correlation measures. The honest claim is
//! therefore "different, but not worse", and this module is what makes the "not worse"
//! falsifiable.
//!
//! The metric is implemented **once**, here, and applied to both arms' positions. A
//! metric restated in the oracle harness as well would be two implementations whose
//! disagreement would be indistinguishable from a layout disagreement.

use libm::sqrt;

/// Max-min pivots in the correlation, or every node when the graph has fewer.
pub const PIVOTS: usize = 32;

/// An undirected graph over dense indices, as both arms lay out: parallel edges
/// collapsed, self-loops dropped (the same collapse `SimpleGraph` performs inside the
/// layout, so both sides see one graph).
#[derive(Debug, Clone, Default)]
pub struct Graph {
    adj: Vec<Vec<u32>>,
}

impl Graph {
    /// Builds from simple edges, each `[lo, hi]` with `lo < hi`, in any order. Node
    /// count is the highest index any edge names, plus one — a lone node with no edge
    /// is a graph of one node the caller must say so by naming it (`with_nodes`).
    pub fn from_edges(edges: &[[u32; 2]]) -> Self {
        let n = edges
            .iter()
            .flat_map(|e| e.iter().copied())
            .max()
            .map_or(0, |m| m as usize + 1);
        let mut graph = Self::with_nodes(n);
        for &[a, b] in edges {
            graph.adj[a as usize].push(b);
            graph.adj[b as usize].push(a);
        }
        graph
    }

    /// A graph of `n` nodes and no edges, for a node count no edge implies.
    pub fn with_nodes(n: usize) -> Self {
        Self {
            adj: vec![Vec::new(); n],
        }
    }

    /// Nodes in the graph.
    pub fn len(&self) -> usize {
        self.adj.len()
    }

    /// True when the graph has no nodes.
    pub fn is_empty(&self) -> bool {
        self.adj.is_empty()
    }

    /// Hop distance from `pivot`; `-1` where unreachable (another component).
    ///
    /// BFS from a single source, so a component's own diameter is what is measured and
    /// a cross-component pair never contributes.
    fn hops(&self, pivot: u32) -> Vec<i64> {
        let mut dist = vec![-1i64; self.len()];
        dist[pivot as usize] = 0;
        let mut queue = std::collections::VecDeque::from([pivot]);
        while let Some(u) = queue.pop_front() {
            for &v in &self.adj[u as usize] {
                if dist[v as usize] < 0 {
                    dist[v as usize] = dist[u as usize] + 1;
                    queue.push_back(v);
                }
            }
        }
        dist
    }

    /// The max-min (farthest-point) pivots: node 0, then repeatedly the node whose
    /// distance to its *nearest* already-chosen pivot is largest, ties keeping the
    /// lower index. An unreachable node counts as infinitely far, so a disconnected
    /// graph picks up a pivot in every component before refining within one.
    fn pivots(&self) -> Vec<u32> {
        let count = PIVOTS.min(self.len());
        let mut chosen = vec![0u32];
        let mut taken = vec![false; self.len()];
        taken[0] = true;
        let reach = |d: i64| if d < 0 { f64::INFINITY } else { d as f64 };
        let mut min: Vec<f64> = self.hops(0).into_iter().map(reach).collect();
        while chosen.len() < count {
            let best = (0..self.len())
                .filter(|&v| !taken[v])
                .max_by(|&a, &b| min[a].total_cmp(&min[b]).then(b.cmp(&a)));
            let Some(best) = best else { break };
            chosen.push(best as u32);
            taken[best] = true;
            for (v, d) in self.hops(best as u32).into_iter().enumerate() {
                min[v] = min[v].min(reach(d));
            }
        }
        chosen
    }
}

/// The stress correlation of `positions` over `graph`: `None` when it does not exist —
/// a graph too small for a pair, or positions that do not cover it. `None` is reported
/// as absent and never fabricated into a 0 or a 1, because a fabricated correlation
/// would enter the margin arithmetic as a real number.
pub fn correlate(graph: &Graph, positions: &[(f64, f64)]) -> Option<f64> {
    if graph.is_empty() || positions.len() != graph.len() {
        return None;
    }
    let (mut hops, mut euclid) = (Vec::new(), Vec::new());
    for p in graph.pivots() {
        let dist = graph.hops(p);
        for v in 0..graph.len() {
            if v as u32 == p || dist[v] < 0 {
                continue;
            }
            hops.push(dist[v] as f64);
            let (px, py) = positions[p as usize];
            let (vx, vy) = positions[v];
            euclid.push(sqrt((vx - px) * (vx - px) + (vy - py) * (vy - py)));
        }
    }
    pearson(&hops, &euclid)
}

/// Pearson's `r`, summed in ascending index order (D3). `None` when either side has no
/// spread: that is a correlation which does not exist, not a correlation of zero.
fn pearson(xs: &[f64], ys: &[f64]) -> Option<f64> {
    if xs.len() < 2 || xs.len() != ys.len() {
        return None;
    }
    let n = xs.len() as f64;
    let (mut mx, mut my) = (0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        mx += x;
        my += y;
    }
    mx /= n;
    my /= n;
    let (mut sxy, mut sxx, mut syy) = (0.0, 0.0, 0.0);
    for (x, y) in xs.iter().zip(ys) {
        let (dx, dy) = (x - mx, y - my);
        sxy += dx * dy;
        sxx += dx * dx;
        syy += dy * dy;
    }
    let den = sqrt(sxx * syy);
    (den > 0.0).then(|| sxy / den)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A path 0-1-2-...-n-1: hop distance from node 0 is the index itself, so a layout
    /// that spreads the nodes evenly must correlate perfectly.
    fn path(n: u32) -> Graph {
        Graph::from_edges(
            &(0..n.saturating_sub(1))
                .map(|i| [i, i + 1])
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn a_layout_that_spreads_a_path_by_hop_distance_correlates_perfectly() {
        let n = 20;
        let positions: Vec<(f64, f64)> = (0..n).map(|i| (f64::from(i), 0.0)).collect();
        let r = correlate(&path(n), &positions).expect("a correlation");
        assert!((r - 1.0).abs() < 1e-12, "{r}");
    }

    #[test]
    fn a_layout_that_piles_a_path_on_one_point_has_no_correlation_to_report() {
        let n = 20;
        let positions = vec![(0.0, 0.0); n as usize];
        assert_eq!(correlate(&path(n), &positions), None, "no euclidean spread");
    }

    #[test]
    fn the_correlation_is_the_pivot_sets_and_not_a_single_pairs() {
        // A star: every leaf is one hop from the centre and two from every other leaf,
        // so the correlation is small however it is drawn, and must not be 1.0 by
        // accident of a single well-placed pair.
        let mut edges = vec![[0u32, 1]];
        edges.extend((2..12).map(|i| [0, i]));
        let graph = Graph::from_edges(&edges);
        let positions: Vec<(f64, f64)> = (0..12)
            .map(|i| (f64::from(i % 3), f64::from(i / 3)))
            .collect();
        let r = correlate(&graph, &positions).expect("a correlation");
        assert!(r < 0.9, "a star must not correlate perfectly: {r}");
    }

    #[test]
    fn an_unreachable_component_is_skipped_not_measured() {
        // A path 0-1-2 and a disjoint edge 3-4. Every cross-component pair is
        // unreachable, so it is skipped rather than measured — a distance of infinity
        // would make pearson's sums NaN, and skipping is the only reason the number
        // below exists at all.
        let graph = Graph::from_edges(&[[0, 1], [1, 2], [3, 4]]);
        let near = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (0.0, 0.0), (5.0, 0.0)];
        let r = correlate(&graph, &near).expect("a correlation");
        assert!(r.is_finite(), "{r}");
        // Translated the second component a very long way from the first, *rigidly*:
        // every distance inside it, and inside the first, is unchanged. If a
        // cross-component pair entered the sums, its distance would grow by 900 and the
        // correlation with it. It is bit-identical, which is the property being pinned.
        let far = [
            (0.0, 0.0),
            (1.0, 0.0),
            (2.0, 0.0),
            (900.0, 0.0),
            (905.0, 0.0),
        ];
        assert_eq!(
            correlate(&graph, &far),
            Some(r),
            "cross-component pairs are skipped"
        );
    }

    #[test]
    fn positions_with_no_spread_report_no_correlation_rather_than_zero() {
        // Every same-component pair is the same distance, so pearson's denominator is
        // zero. That is a correlation which does not exist; reporting 0 would let it
        // enter the margin arithmetic as a real measurement.
        let graph = Graph::from_edges(&[[0, 1], [2, 3]]);
        let positions = [(0.0, 0.0), (1.0, 0.0), (0.0, 0.0), (1.0, 0.0)];
        assert_eq!(correlate(&graph, &positions), None);
    }

    #[test]
    fn a_single_node_has_no_pair_and_no_correlation() {
        let graph = Graph::with_nodes(1);
        assert_eq!(correlate(&graph, &[(1.0, 2.0)]), None);
        assert_eq!(
            correlate(&graph, &[]),
            None,
            "positions must cover the graph"
        );
    }

    #[test]
    fn pivots_start_at_node_zero_and_take_the_furthest_from_their_nearest() {
        let graph = path(10);
        let pivots = graph.pivots();
        assert_eq!(pivots[0], 0, "the first pivot is fixed at dense index 0");
        assert_eq!(pivots.len(), PIVOTS.min(10));
        assert_eq!(
            pivots[1], 9,
            "the far end of a path is furthest from node 0"
        );
        let mut sorted = pivots.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), pivots.len(), "no pivot is chosen twice");
    }

    #[test]
    fn a_graph_smaller_than_the_pivot_count_uses_every_node() {
        let graph = path(4);
        assert_eq!(graph.pivots(), vec![0, 3, 1, 2]);
    }
}
