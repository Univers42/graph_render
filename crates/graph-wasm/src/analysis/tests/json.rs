//! The exact JSON text, for every analysis, over two fixtures.

use super::fixtures::*;
use graph_contract::canonical_json::parse;
use graph_core::analysis::{components, depth};
use graph_core::layout::hierarchy::Hierarchy;

#[test]
fn the_json_face_is_pinned_byte_for_byte() {
    let t = path();
    let want = [
        (
            "analysis.components.weak",
            r#"{"id":"analysis.components.weak","kind":"u32","nodeCount":3,"values":[0,0,0]}"#,
        ),
        (
            "analysis.components.strong",
            r#"{"id":"analysis.components.strong","kind":"u32","nodeCount":3,"values":[0,0,0]}"#,
        ),
        (
            // A path has no community structure to find, so every node stays in one
            // community and modularity is exactly 0 — graph-core's own `one_lump`
            // property, restated through the ABI.
            "analysis.communities.louvain",
            r#"{"id":"analysis.communities.louvain","kind":"u32","modularity":0,"nodeCount":3,"values":[0,0,0]}"#,
        ),
        (
            "analysis.centrality.degree",
            r#"{"id":"analysis.centrality.degree","kind":"f64","nodeCount":3,"values":[1,2,1]}"#,
        ),
        (
            // Unit-strength edges weigh 0.5 each, so `a` reaches `b` at 0.5 and `c` at
            // 1.0: reachable 2 of 2, sum 1.5, and 2/1.5 scaled by 1 is 4/3.
            "analysis.centrality.closeness",
            r#"{"id":"analysis.centrality.closeness","kind":"f64","nodeCount":3,"values":[1.3333333730697632,2,1.3333333730697632]}"#,
        ),
        (
            "analysis.centrality.betweenness",
            r#"{"id":"analysis.centrality.betweenness","kind":"f64","nodeCount":3,"values":[0,2,0]}"#,
        ),
        (
            // A path is bipartite, so the power iteration oscillates between the two
            // sides and never settles: graph-core returns the last normalised iterate
            // and `converged: false`, which is the escape hatch its own `Ponytail` marker
            // names. The ABI carries that flag rather than dropping it — a caller told
            // only the numbers would rank three equal values as a real centrality.
            "analysis.centrality.eigenvector",
            r#"{"converged":false,"id":"analysis.centrality.eigenvector","kind":"f64","nodeCount":3,"values":[0.5773502588272095,0.5773502588272095,0.5773502588272095]}"#,
        ),
        (
            // The path fixture's edges are `relation`, not `hierarchy`, so the hierarchy
            // has no parent edge at all and every node is a root: three roots means the
            // virtual root, and every real root is at depth 1. The two-root and one-root
            // cases are pinned in `depth_follows_the_virtual_root_convention_over_a_forest`.
            "analysis.depth.bfs",
            r#"{"id":"analysis.depth.bfs","kind":"u32","max":1,"nodeCount":3,"values":[1,1,1]}"#,
        ),
    ];
    assert_eq!(want.len(), crate::analysis::registry::count() as usize);
    for (id, text) in want {
        let index = index_of(id);
        assert_eq!(crate::analysis::registry::to_json(index, &t).as_deref(), Some(text), "{id}");
    }
}

#[test]
fn weak_and_strong_components_are_pinned_on_a_directed_two_cycle() {
    let t = two_cycle();
    assert_eq!(
        crate::analysis::registry::to_json(0, &t).as_deref(),
        Some(r#"{"id":"analysis.components.weak","kind":"u32","nodeCount":2,"values":[0,0]}"#)
    );
    assert_eq!(
        crate::analysis::registry::to_json(1, &t).as_deref(),
        Some(r#"{"id":"analysis.components.strong","kind":"u32","nodeCount":2,"values":[0,0]}"#)
    );
    // A one-way edge is the case the two must differ on: strong splits it, weak does not.
    let one_way = directed(&["a", "b"], &[("e0", "a", "b")]);
    assert_eq!(components::strong(&one_way), [0, 1]);
    assert_eq!(components::weak(&one_way), [0, 0]);
}

#[test]
fn depth_follows_the_virtual_root_convention_over_a_forest() {
    let t = forest();
    assert_eq!(
        depth::bfs_depth(&Hierarchy::of(&t).expect("repairs")).levels(),
        &[1, 2, 3, 1]
    );
    assert_eq!(
        crate::analysis::registry::to_json(7, &t).as_deref(),
        Some(
            r#"{"id":"analysis.depth.bfs","kind":"u32","max":3,"nodeCount":4,"values":[1,2,3,1]}"#
        )
    );
    // A graph whose only edge is a `relation` has no hierarchy edge at all, so *every*
    // node is a root: two roots means the virtual root, and both sit at depth 1. This is
    // the case that would read as "an orphan at depth 0" if the convention were skipped.
    let single = topology(&["a", "b"], &[("e0", "a", "b")]);
    assert_eq!(
        crate::analysis::registry::to_json(7, &single).as_deref(),
        Some(r#"{"id":"analysis.depth.bfs","kind":"u32","max":1,"nodeCount":2,"values":[1,1]}"#),
        "no hierarchy edge: every node is a root, and two roots hang off the virtual one"
    );
    // One root is at depth 0 — the difference between the two root cases, pinned.
    let chain = forest_without_the_isolated_node();
    assert_eq!(
        crate::analysis::registry::to_json(7, &chain).as_deref(),
        Some(r#"{"id":"analysis.depth.bfs","kind":"u32","max":2,"nodeCount":3,"values":[0,1,2]}"#),
        "a single root counts out from itself"
    );
}

#[test]
fn an_empty_graph_answers_an_empty_column_from_every_analysis() {
    let t = graph_core::empty_model();
    for id in ids() {
        let index = index_of(id);
        let report = crate::analysis::registry::run(index, &t).expect("registered");
        assert_eq!(report.values.len(), 0, "{id}");
        let text = crate::analysis::registry::to_json(index, &t).expect("encodes");
        let parsed = parse(&text).expect("the face is valid JSON");
        assert_eq!(count_of(&parsed), Some(0), "{id}: {text}");
    }
}

#[test]
fn the_face_parses_and_its_keys_are_in_ascending_order() {
    let t = forest();
    for id in ids() {
        let index = index_of(id);
        let text = crate::analysis::registry::to_json(index, &t).expect("encodes");
        let graph_contract::canonical_json::Value::Object(members) = parse(&text).expect("the face is valid JSON") else {
            panic!("{id}: not an object");
        };
        let keys: Vec<&str> = members.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "{id}: keys ascend, so the text is canonical");
        assert_eq!(
            count_of(&graph_contract::canonical_json::Value::Object(members.clone())),
            Some(u64::from(t.node_count())),
            "{id}: nodeCount is the topology's node count, as a number"
        );
        assert!(
            members.iter().any(|(k, _)| k == "id"),
            "{id}: every result names itself"
        );
    }
}