use super::*;

/// The two directed edges of a two-cycle. Both nodes are one weak component and, because
/// each reaches the other, one strong component too — the pin that would fail if the
/// strong row were quietly the weak one.
#[test]
fn weak_and_strong_components_are_pinned_on_a_directed_two_cycle() {
    let t = two_cycle();
    assert_eq!(
        to_json(0, &t).as_deref(),
        Some(r#"{"id":"analysis.components.weak","kind":"u32","nodeCount":2,"values":[0,0]}"#)
    );
    assert_eq!(
        to_json(1, &t).as_deref(),
        Some(r#"{"id":"analysis.components.strong","kind":"u32","nodeCount":2,"values":[0,0]}"#)
    );
    // A one-way edge is the case the two must differ on: strong splits it, weak does not.
    let one_way = directed(&["a", "b"], &[("e0", "a", "b")]);
    assert_eq!(components::strong(&one_way), [0, 1]);
    assert_eq!(components::weak(&one_way), [0, 0]);
}

/// Depth follows D-H's convention, including the virtual root: with two real roots every
/// real root is at depth 1 and `max` is 2 for the deepest chain below one of them. An
/// isolated node counts as a root, which is what puts `z` at 1.
#[test]
fn depth_follows_the_virtual_root_convention_over_a_forest() {
    let t = forest();
    assert_eq!(
        depth::bfs_depth(&Hierarchy::of(&t).expect("repairs")).levels(),
        &[1, 2, 3, 1]
    );
    assert_eq!(
        to_json(7, &t).as_deref(),
        Some(
            r#"{"id":"analysis.depth.bfs","kind":"u32","max":3,"nodeCount":4,"values":[1,2,3,1]}"#
        )
    );
    // A graph whose only edge is a `relation` has no hierarchy edge at all, so *every*
    // node is a root: two roots means the virtual root, and both sit at depth 1. This is
    // the case that would read as "an orphan at depth 0" if the convention were skipped.
    let single = topology(&["a", "b"], &[("e0", "a", "b")]);
    assert_eq!(
        to_json(7, &single).as_deref(),
        Some(r#"{"id":"analysis.depth.bfs","kind":"u32","max":1,"nodeCount":2,"values":[1,1]}"#),
        "no hierarchy edge: every node is a root, and two roots hang off the virtual one"
    );
    // One root is at depth 0 — the difference between the two root cases, pinned.
    let chain = forest_without_the_isolated_node();
    assert_eq!(
        to_json(7, &chain).as_deref(),
        Some(r#"{"id":"analysis.depth.bfs","kind":"u32","max":2,"nodeCount":3,"values":[0,1,2]}"#),
        "a single root counts out from itself"
    );
}

/// A graph with no nodes is a real case, not a panic: every analysis answers an empty
/// column of length 0.
#[test]
fn an_empty_graph_answers_an_empty_column_from_every_analysis() {
    let t = graph_core::empty_model();
    for id in ids() {
        let index = index_of(id);
        let report = run(index, &t).expect("registered");
        assert_eq!(report.values.len(), 0, "{id}");
        let text = to_json(index, &t).expect("encodes");
        let parsed = parse(&text).expect("the face is valid JSON");
        assert_eq!(count_of(&parsed), Some(0), "{id}: {text}");
    }
}
