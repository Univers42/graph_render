//! Registry structure tests.

use super::fixtures::*;

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
    assert_eq!(crate::analysis::registry::count(), 8);
    for (i, id) in ids().iter().enumerate() {
        assert_eq!(
            crate::analysis::registry::id_at(u32::try_from(i).expect("small")),
            Some(*id)
        );
    }
    assert_eq!(
        crate::analysis::registry::id_at(8),
        None,
        "one past the end is refused, not a panic"
    );
    assert_eq!(crate::analysis::registry::id_at(u32::MAX), None);
}

#[test]
fn running_or_encoding_past_the_end_is_refused_rather_than_answered_with_another_row() {
    let t = path();
    assert!(crate::analysis::registry::run(8, &t).is_none());
    assert!(crate::analysis::registry::run(u32::MAX, &t).is_none());
    assert!(crate::analysis::registry::to_json(8, &t).is_none());
    assert!(crate::analysis::registry::to_json(u32::MAX, &t).is_none());
    assert!(
        crate::analysis::registry::run(0, &t).is_some(),
        "row 0 itself runs"
    );
}
