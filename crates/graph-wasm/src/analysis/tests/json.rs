use super::*;

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
