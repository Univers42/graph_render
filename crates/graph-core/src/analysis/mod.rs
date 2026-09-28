//! ANALYSIS stage (`prompts/phase-07-analysis.md`): components, shortest paths,
//! centrality, community detection — all pure functions over the existing
//! [`crate::index::Topology`], no new graph representation, no I/O, no wall-clock (D9).
//!
//! `depth` (hierarchy-derived depth) is **not** registered here: it needs p3's
//! `hierarchy.rs`, which is not on this branch's base. A recorded deviation
//! (`docs/measurements/phase07-analysis.md`), not an improvisation — `analysis.depth`
//! stays absent from the capability ledger until the merge step.
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
pub mod paths;

#[cfg(test)]
mod determinism {
    use crate::index::index_model;
    use crate::records::build::{edge, node};

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
    }
}
