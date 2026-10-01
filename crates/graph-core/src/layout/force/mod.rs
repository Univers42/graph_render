//! Force-directed layout: Barnes-Hut approximated many-body, plus the Jacobi/gather
//! ports of d3's link and collide (devil C7), driven either as the frozen one-shot
//! ([`BarnesHut`], which is a default [`session::ForceSession`]) or as a live
//! [`session::ForceSession`], plus [`spring`] — a dense Fruchterman–Reingold port, which
//! shares neither Barnes-Hut's theta-tree nor FA2's cumulative swing, and is published
//! because SciGraphs' `SPRING` is that algorithm. `prompt.md` §3.1, Phase 6 branch p6f.

pub(crate) mod barnes_hut;
pub mod davidson_harel;
pub mod drl;
pub mod fruchterman_reingold;
pub mod graphopt;
pub mod kamada_kawai;
pub mod lgl;
pub(crate) mod params;
pub(crate) mod quadtree;
pub(crate) mod session;
pub mod spring;
pub(crate) mod yifan_hu;

pub use barnes_hut::{BarnesHut, Split};
pub use davidson_harel::DavidsonHarel;
pub use drl::Drl;
pub use fruchterman_reingold::FruchtermanReingold;
pub use graphopt::Graphopt;
pub use kamada_kawai::KamadaKawai;
pub use lgl::Lgl;
pub use params::ForceParams;
pub use session::{ForceSession, LiveParams, NodeRow, SessionError, StepReport};
pub use yifan_hu::YifanHu;

use crate::arena::FixedState;
use crate::csr::Csr;
use crate::index::Topology;
use indexmap::IndexMap;

/// The undirected, deduplicated, self-loop-free adjacency every force layout shares
/// (devil C6): a topology may hold parallel edges (the synthetic model's `earlier` draws
/// can repeat, and `NoteLink` extras can duplicate an existing pair) and self-loops.
/// Collapsing them once here, instead of in each consumer, is what keeps FA2's
/// `mass = degree + 1` and the link force's degree-weighted split looking at the same
/// graph. Convention, arbitrary but stated (as `layout/grid.rs`'s own conventions are):
/// among the raw edges that collapse onto one unordered pair, the **first by ascending
/// edge index** keeps its `strength`; the multiplicity itself is discarded, never
/// averaged or summed.
#[derive(Debug, Clone)]
pub(crate) struct SimpleGraph {
    /// Each simple edge's lower-index endpoint.
    pub(crate) lo: Vec<u32>,
    /// Each simple edge's higher-index endpoint.
    pub(crate) hi: Vec<u32>,
    /// The surviving raw edge's strength, per simple edge.
    pub(crate) strength: Vec<f64>,
    /// Node → its simple-edge indices (both endpoints' rows), arrival order.
    pub(crate) rows: Csr,
}

impl SimpleGraph {
    /// Node `v`'s degree in this simple graph: how many distinct neighbours it has.
    pub(crate) fn degree(&self, v: u32) -> u32 {
        self.rows.row(v).len() as u32
    }

    /// The endpoint of simple edge `e` that is not `v` — the neighbour, for a kernel
    /// walking one node's row. `e` must be in `v`'s row (or the same edge read the other
    /// way round); a self-loop never is, since `simple_graph` drops those.
    pub(crate) fn other(&self, e: u32, v: u32) -> u32 {
        if self.lo[e as usize] == v {
            self.hi[e as usize]
        } else {
            self.lo[e as usize]
        }
    }
}

/// Builds [`SimpleGraph`] from `t`'s kept edges, in edge-index order.
pub(crate) fn simple_graph(t: &Topology) -> SimpleGraph {
    let mut seen: IndexMap<(u32, u32), u32, FixedState> = IndexMap::default();
    let (mut lo, mut hi, mut strength) = (Vec::new(), Vec::new(), Vec::new());
    let edges = t.edges();
    for e in 0..t.edge_count() as usize {
        let (a, b) = (edges.source[e], edges.target[e]);
        if a == b {
            continue;
        }
        let pair = (a.min(b), a.max(b));
        if seen.contains_key(&pair) {
            continue;
        }
        seen.insert(pair, lo.len() as u32);
        lo.push(pair.0);
        hi.push(pair.1);
        strength.push(edges.strength[e]);
    }
    SimpleGraph::from_edges(t.node_count(), lo, hi, strength)
}

impl SimpleGraph {
    /// A simple graph over `n` nodes from already-deduplicated edges, `lo[e] < hi[e]`.
    pub(crate) fn from_edges(n: u32, lo: Vec<u32>, hi: Vec<u32>, strength: Vec<f64>) -> Self {
        let rows = row_csr(n, &lo, &hi);
        SimpleGraph {
            lo,
            hi,
            strength,
            rows,
        }
    }
}

/// Every simple edge filed under both its endpoints, arrival (simple-edge-index) order.
fn row_csr(n: u32, lo: &[u32], hi: &[u32]) -> Csr {
    let pairs = (0..lo.len() as u32).flat_map(|e| [(lo[e as usize], e), (hi[e as usize], e)]);
    Csr::from_pairs(n, pairs).expect("edge count fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    #[test]
    fn parallel_edges_collapse_and_self_loops_drop() {
        let nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [
            edge("e1", "a", "b"),
            edge("e2", "b", "a"), // parallel, reversed — same unordered pair
            edge("e3", "c", "c"), // self-loop
            edge("e4", "b", "c"),
        ];
        let t = index_model(&nodes, &edges).expect("fits");
        let g = simple_graph(&t);
        assert_eq!((g.lo.clone(), g.hi.clone()), (vec![0, 1], vec![1, 2]));
        assert_eq!(g.strength, [edges[0].strength, edges[3].strength]);
        assert_eq!((g.degree(0), g.degree(1), g.degree(2)), (1, 2, 1));
    }

    #[test]
    fn an_empty_graph_has_an_empty_simple_graph() {
        let t = crate::index::empty_model();
        let g = simple_graph(&t);
        assert!(g.lo.is_empty() && g.rows.is_empty());
    }
}
