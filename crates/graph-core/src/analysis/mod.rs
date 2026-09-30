//! ANALYSIS stage (`prompts/phase-07-analysis.md`): components, shortest paths,
//! centrality, community detection, hierarchy depth — all pure functions over the
//! existing [`crate::index::Topology`], no new graph representation, no I/O, no
//! wall-clock (D9).
//!
//! [`depth`] is the one that crosses a crate boundary: it reads the root/forest
//! convention through its own [`depth::Roots`] trait, and p3's
//! [`layout::hierarchy::Hierarchy`](crate::layout::hierarchy::Hierarchy) implements
//! that trait by delegation — `impl Roots for Hierarchy {}` is the whole of the
//! re-point, and it has landed. `analysis.depth` now has its ledger row; there is one
//! convention across the codebase, not two.
//!
//! Exposing these results in the snapshot/JSON/SDK, and folding them into the 4-way
//! hashgate as their own stage, is deferred to the merge step (`graph-wasm` and the
//! hashgate's `STAGES` array are outside this phase's authorization envelope). Every
//! function here is still exercised by [`analysis_determinism`] below: same input,
//! same output, bit for bit, on this target — the property the hashgate would check
//! across targets once wired in.

pub mod centrality;
pub mod communities;
pub mod components;
pub mod depth;
pub mod paths;

#[cfg(test)]
mod determinism {
    use crate::analysis::depth::{self, Roots};
    use crate::index::{Topology, index_model};
    use crate::records::build::{edge, node};

    /// The hierarchy CSR read as it stands — **no repair**. Enough to give `depth` a
    /// [`Roots`] here, where the only property under test is that two runs agree; the
    /// depth convention itself is pinned in `analysis/depth/tests.rs`, and the repaired
    /// forest is p3's `Hierarchy`, which now implements `Roots` directly.
    struct AsIs {
        topology: Topology,
        children: Vec<Vec<u32>>,
        roots: Vec<u32>,
    }

    impl AsIs {
        /// A node with no hierarchy edge leaving it is a root, ascending.
        fn of(topology: &Topology) -> Self {
            let children: Vec<Vec<u32>> = (0..topology.node_count())
                .map(|v| {
                    topology
                        .hierarchy()
                        .row(v)
                        .iter()
                        .map(|&e| topology.edges().target[e as usize])
                        .collect()
                })
                .collect();
            let roots = (0..topology.node_count())
                .filter(|&v| children[v as usize].is_empty())
                .collect();
            Self {
                topology: topology.clone(),
                children,
                roots,
            }
        }
    }

    impl Roots for AsIs {
        fn node_count(&self) -> u32 {
            self.topology.node_count()
        }

        fn roots(&self) -> &[u32] {
            &self.roots
        }

        fn virtual_root(&self) -> Option<u32> {
            (self.roots.len() >= 2).then_some(self.topology.node_count())
        }

        fn children(&self, v: u32) -> &[u32] {
            &self.children[v as usize]
        }
    }

    /// Every analysis function, run twice over the same input, must agree bit for bit —
    /// the in-target half of the property the hashgate checks across targets. This
    /// stands in for that gate until analysis is wired into it at the merge step
    /// (module doc: deferred hashgate exposure).
    #[test]
    fn analysis_determinism() {
        let nodes = [node("a", ""), node("b", ""), node("c", ""), node("d", "")];
        let edges = [
            edge("ab", "a", "b"),
            edge("bc", "b", "c"),
            edge("cd", "c", "d"),
        ];
        let t = index_model(&nodes, &edges).expect("fits");

        assert_eq!(super::components::weak(&t), super::components::weak(&t));
        assert_eq!(super::components::strong(&t), super::components::strong(&t));
        assert_eq!(
            super::paths::dijkstra_distances(&t, 0),
            super::paths::dijkstra_distances(&t, 0)
        );
        assert_eq!(
            super::paths::bellman_ford(&t, 0),
            super::paths::bellman_ford(&t, 0)
        );
        assert_eq!(super::centrality::degree(&t), super::centrality::degree(&t));
        assert_eq!(
            super::centrality::closeness(&t),
            super::centrality::closeness(&t)
        );
        assert_eq!(
            super::centrality::betweenness(&t),
            super::centrality::betweenness(&t)
        );
        assert_eq!(
            super::centrality::eigenvector(&t),
            super::centrality::eigenvector(&t)
        );
        assert_eq!(
            super::communities::louvain(&t),
            super::communities::louvain(&t)
        );
        let forest = AsIs::of(&t);
        assert_eq!(
            depth::bfs_depth(&forest),
            depth::bfs_depth(&forest),
            "depth reads the forest, not a hash order"
        );
        assert_eq!(
            depth::depth_from(&forest, &[0]),
            depth::depth_from(&forest, &[0])
        );
    }
}
