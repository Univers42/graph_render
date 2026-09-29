//! Row correctness tests: every row calls the graph-core function its id names.

use super::fixtures::*;
use graph_core::analysis::{centrality, communities, components};
use graph_core::layout::hierarchy::Hierarchy;

#[test]
fn every_row_calls_the_graph_core_function_its_id_names() {
    for t in [path(), directed_one_way(), forest()] {
        every_row_over(&t);
    }
}

fn every_row_over(t: &graph_core::Topology) {
    for (i, id) in ids().iter().enumerate() {
        let index = u32::try_from(i).expect("small");
        let report = crate::analysis::registry::run(index, t).expect("registered");
        assert_eq!(report.id, *id);
        match *id {
            "analysis.components.weak" => {
                assert_eq!(report.values, crate::analysis::report::Column::U32(components::weak(t)))
            }
            "analysis.components.strong" => {
                assert_eq!(report.values, crate::analysis::report::Column::U32(components::strong(t)))
            }
            "analysis.communities.louvain" => {
                let labels = communities::louvain(t);
                assert_eq!(report.values, crate::analysis::report::Column::U32(labels.clone()));
                assert_eq!(report.modularity, Some(communities::modularity(t, &labels)));
            }
            "analysis.centrality.degree" => {
                let want: Vec<f64> = centrality::degree(t)
                    .iter()
                    .map(|&d| f64::from(d))
                    .collect();
                assert_eq!(report.values, crate::analysis::report::Column::F64(want));
            }
            "analysis.centrality.closeness" => assert_eq!(
                report.values,
                crate::analysis::report::Column::F64(widened(centrality::closeness(t)))
            ),
            "analysis.centrality.betweenness" => assert_eq!(
                report.values,
                crate::analysis::report::Column::F64(widened(centrality::betweenness(t)))
            ),
            "analysis.centrality.eigenvector" => {
                let (values, converged) = centrality::eigenvector(t);
                assert_eq!(report.values, crate::analysis::report::Column::F64(widened(values)));
                assert_eq!(report.converged, Some(converged));
            }
            "analysis.depth.bfs" => {
                // Cross-checked against `Hierarchy`'s own depth column, never against
                // `forest_depth`: comparing this row with the function it calls would
                // agree with whatever that function returned. `Hierarchy` counts
                // breadth-first from the (possibly virtual) root, which is the same
                // convention `depth::bfs_depth` implements — so agreement is the
                // one-convention claim the re-point rests on, and disagreement is real.
                let hierarchy = Hierarchy::of(t).expect("the hierarchy repairs");
                let want: Vec<u32> = (0..t.node_count()).map(|v| hierarchy.depth(v)).collect();
                assert_eq!(report.values, crate::analysis::report::Column::U32(want));
                assert_eq!(report.max, Some(hierarchy.max_depth()));
            }
            other => panic!("{other} is not one of the pinned ids"),
        }
    }
}

/// `f32` as graph-core states a centrality, widened to the `f64` the wire carries. Exact:
/// every `f32` is a `f64`, so this loses nothing and changes no comparison.
fn widened(values: Vec<f32>) -> Vec<f64> {
    values.into_iter().map(f64::from).collect()
}