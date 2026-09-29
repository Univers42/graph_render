//! The ABI's ANALYSIS registry and its JSON face, over graph-core's own functions.
//! Natively testable because `super` is: the table is plain data, every entry point is a
//! pure function over a [`Topology`], and the JSON is written here rather than through a
//! wasm pointer.

use super::*;
use crate::ingest;
use graph_contract::canonical_json::{Value, parse};
use graph_core::analysis::{centrality, communities, components};
use graph_core::index_model;
use graph_core::layout::hierarchy::Hierarchy;

fn node_json(id: &str) -> String {
    format!(
        r#"{{"id":"{id}","kind":"record","database_id":null,"source":"pg","label":"L","group":null,"weight":0.5,"version":0.0,"has_note":false,"icon":null}}"#
    )
}

fn edge_json(id: &str, source: &str, target: &str) -> String {
    directed_edge_json(id, source, target, false)
}

/// The same edge, direction respected or not. A `relation` edge is undirected unless the
/// document says `directed: true`, so the two kinds of component can only be told apart
/// on a fixture that actually sets it.
fn directed_edge_json(id: &str, source: &str, target: &str, directed: bool) -> String {
    format!(
        r#"{{"id":"{id}","source":"{source}","target":"{target}","kind":"relation","label":"","strength":0.5,"directed":{directed},"record_id":null}}"#
    )
}

/// A topology through the same reader `gm_build` uses, so these tests need no second
/// way of making a graph.
fn topology(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<String> = node_ids.iter().map(|id| node_json(id)).collect();
    let edges: Vec<String> = edges.iter().map(|(id, s, t)| edge_json(id, s, t)).collect();
    indexed(&nodes, &edges)
}

/// The same, with every edge directed.
fn directed(node_ids: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let nodes: Vec<String> = node_ids.iter().map(|id| node_json(id)).collect();
    let edges: Vec<String> = edges
        .iter()
        .map(|(id, s, t)| directed_edge_json(id, s, t, true))
        .collect();
    indexed(&nodes, &edges)
}

fn indexed(nodes: &[String], edges: &[String]) -> Topology {
    let text = format!(
        r#"{{"version":1,"nodes":[{}],"edges":[{}]}}"#,
        nodes.join(","),
        edges.join(",")
    );
    let (nodes, edges) = ingest::read(text.as_bytes()).expect("the fixture is valid");
    index_model(&nodes, &edges).expect("the fixture indexes")
}

/// A path `a - b - c`: `b` is an articulation point, so components, communities and
/// depth all have something to say, and every value below is derivable by hand.
fn path() -> Topology {
    topology(&["a", "b", "c"], &[("e0", "a", "b"), ("e1", "b", "c")])
}

/// The two directed edges `a -> b` and `b -> a`: the only shape on which strong
/// components can differ from weak ones.
fn two_cycle() -> Topology {
    directed(&["a", "b"], &[("e0", "a", "b"), ("e1", "b", "a")])
}

/// A single directed `a -> b` and nothing else: weak joins the two, strong does not, and
/// the hierarchy is empty so both nodes are roots.
fn directed_one_way() -> Topology {
    directed(&["a", "b"], &[("e0", "a", "b")])
}

/// A parent-child edge, which is what the hierarchy CSR reads — `kind: "hierarchy"`
/// with the parent as source (D-Q1: only `child_of` is flipped).
fn tree_json(id: &str, parent: &str, child: &str) -> String {
    format!(
        r#"{{"id":"{id}","source":"{parent}","target":"{child}","kind":"hierarchy","label":"parent_of","strength":0.5,"directed":true,"record_id":null}}"#
    )
}

/// `a` above `b` above `c`, plus an isolated `z`: the two-roots case, where every real
/// root hangs off the virtual root at dense index `n` and so sits at depth 1 (D-H).
fn forest() -> Topology {
    let nodes: Vec<String> = ["a", "b", "c", "z"]
        .iter()
        .map(|id| node_json(id))
        .collect();
    let edges = vec![tree_json("t0", "a", "b"), tree_json("t1", "b", "c")];
    indexed(&nodes, &edges)
}

/// The same chain without the isolated node, so there is exactly one root.
fn forest_without_the_isolated_node() -> Topology {
    let nodes: Vec<String> = ["a", "b", "c"].iter().map(|id| node_json(id)).collect();
    let edges = vec![tree_json("t0", "a", "b"), tree_json("t1", "b", "c")];
    indexed(&nodes, &edges)
}

fn ids() -> Vec<&'static str> {
    ANALYSES.iter().map(|entry| entry.id).collect()
}

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
                // one-convention claim the re-point makes, and disagreement is real.
                let hierarchy = Hierarchy::of(t).expect("the hierarchy repairs");
                let want: Vec<u32> = (0..t.node_count()).map(|v| hierarchy.depth(v)).collect();
                assert_eq!(report.values, Column::U32(want));
                assert_eq!(report.max, Some(hierarchy.max_depth()));
            }
            other => panic!("{other} is not one of the pinned ids"),
        }
    }
}

/// The exact JSON text, for every analysis, over two fixtures. Every value is derivable
/// by hand from the path fixture (unit edge weights, so closeness is `1/sum of
/// distances` and betweenness counts the two shortest paths through the middle node) —
/// which is what makes this a pin on the *wiring* rather than a recording of whatever
/// the code happened to print.
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
    assert_eq!(want.len(), count() as usize);
    for (id, text) in want {
        let index = index_of(id);
        assert_eq!(to_json(index, &t).as_deref(), Some(text), "{id}");
    }
}

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
        depth::bfs_depth(&Forest(Hierarchy::of(&t).expect("repairs"))).levels(),
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

/// The face is JSON a strict reader accepts — the contract's own parser, not a regex —
/// and its keys are in ascending order, so two runs are byte-comparable.
#[test]
fn the_face_parses_and_its_keys_are_in_ascending_order() {
    let t = forest();
    for id in ids() {
        let index = index_of(id);
        let text = to_json(index, &t).expect("encodes");
        let Value::Object(members) = parse(&text).expect("the face is valid JSON") else {
            panic!("{id}: not an object");
        };
        let keys: Vec<&str> = members.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "{id}: keys ascend, so the text is canonical");
        assert_eq!(
            count_of(&Value::Object(members.clone())),
            Some(u64::from(t.node_count())),
            "{id}: nodeCount is the topology's node count, as a number"
        );
        assert!(
            members.iter().any(|(k, _)| k == "id"),
            "{id}: every result names itself"
        );
    }
}

/// The number of elements the JSON's `nodeCount` member says, or `None`.
fn count_of(value: &Value) -> Option<u64> {
    let Value::Object(members) = value else {
        return None;
    };
    members
        .iter()
        .find(|(k, _)| k == "nodeCount")
        .and_then(|(_, v)| match v {
            Value::Number(text) => text.parse().ok(),
            _ => None,
        })
}

fn index_of(id: &str) -> u32 {
    ids()
        .iter()
        .position(|&candidate| candidate == id)
        .map(|i| u32::try_from(i).expect("small"))
        .unwrap_or_else(|| panic!("{id} is not registered"))
}

/// `kind` is the one member of the face that says whether `values` holds scores or
/// labels, so both of its branches are pinned: an `f64` column and a `u32` one, and the
/// length a `u32` label column reports.
#[test]
fn kind_names_the_element_type_of_both_column_kinds() {
    assert_eq!(Column::F64(vec![0.5, 1.5]).kind(), "f64");
    assert_eq!(Column::U32(vec![0, 1]).kind(), "u32");
    assert_eq!(Column::F64(vec![0.5, 1.5]).len(), 2);
    assert_eq!(Column::U32(vec![0, 1, 2]).len(), 3);
    assert_eq!(Column::U32(vec![]).len(), 0);
}
