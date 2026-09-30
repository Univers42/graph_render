use super::*;

#[test]
fn the_registry_is_the_three_labellings_then_the_four_centralities_then_depth() {
    assert_eq!(
        ids(),
        [
            "analysis.components.weak",
            "analysis.components.strong",
            "analysis.communities.louvain",
            "analysis.centrality.degree",
            "analysis.centrality.closeness",
            "analysis.centrality.betweenness",
            "analysis.centrality.eigenvector",
            "analysis.depth.bfs",
        ]
    );
    assert_eq!(count(), 8);
    for (i, id) in ids().iter().enumerate() {
        assert_eq!(id_at(u32::try_from(i).expect("small")), Some(*id));
    }
    assert_eq!(id_at(8), None, "one past the end is refused, not a panic");
    assert_eq!(id_at(u32::MAX), None);
}

#[test]
fn running_or_encoding_past_the_end_is_refused_rather_than_answered_with_another_row() {
    let t = path();
    assert!(run(8, &t).is_none());
    assert!(run(u32::MAX, &t).is_none());
    assert!(to_json(8, &t).is_none());
    assert!(to_json(u32::MAX, &t).is_none());
    assert!(run(0, &t).is_some(), "row 0 itself runs");
}

/// The registry is a *table of graph-core's functions*, so a row that pointed at another
/// analysis's function would still produce a well-formed report of the right shape and
/// be wrong. Each row is checked against the function it names.
#[test]
fn every_row_calls_the_graph_core_function_its_id_names() {
    for t in [path(), directed_one_way(), forest()] {
        every_row_over(&t);
    }
}

/// Three fixtures, because one of them cannot tell two rows apart:
///
/// - `path()` is undirected, so weak and strong components agree on it — strong
///   components run in the weak row's place would pass on it alone.
/// - `directed_one_way()` is a single `a -> b`: the two differ, and it pins the rest.
/// - `forest()` has a `parent_of` chain *and* two roots, so its real roots sit at depth
///   1 under the virtual root. A depth row that read declared roots instead would put
///   them at 0 and fail here and nowhere else.
fn every_row_over(t: &Topology) {
    for (i, id) in ids().iter().enumerate() {
        let index = u32::try_from(i).expect("small");
        let report = run(index, t).expect("registered");
        assert_eq!(report.id, *id);
        match *id {
            "analysis.components.weak" => {
                assert_eq!(report.values, Column::U32(components::weak(t)))
            }
            "analysis.components.strong" => {
                assert_eq!(report.values, Column::U32(components::strong(t)))
            }
            "analysis.communities.louvain" => {
                let labels = communities::louvain(t);
                assert_eq!(report.values, Column::U32(labels.clone()));
                assert_eq!(report.modularity, Some(communities::modularity(t, &labels)));
            }
            "analysis.centrality.degree" => {
                let want: Vec<f64> = centrality::degree(t)
                    .iter()
                    .map(|&d| f64::from(d))
                    .collect();
                assert_eq!(report.values, Column::F64(want));
            }
            "analysis.centrality.closeness" => assert_eq!(
                report.values,
                Column::F64(widened(centrality::closeness(t)))
            ),
            "analysis.centrality.betweenness" => assert_eq!(
                report.values,
                Column::F64(widened(centrality::betweenness(t)))
            ),
            "analysis.centrality.eigenvector" => {
                let (values, converged) = centrality::eigenvector(t);
                assert_eq!(report.values, Column::F64(widened(values)));
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
                assert_eq!(report.values, Column::U32(want));
                assert_eq!(report.max, Some(hierarchy.max_depth()));
            }
            other => panic!("{other} is not one of the pinned ids"),
        }
    }
}
