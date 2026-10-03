use super::*;
use crate::records::build::{edge, kind, node};

fn ids(topology: &Topology, edges: impl Iterator<Item = u32>) -> Vec<String> {
    edges.map(|e| topology.edge(e).id.to_owned()).collect()
}

#[test]
fn nodes_dedupe_first_wins_in_input_order() {
    let mut late = node("a", "db2");
    late.label = "second".into();
    let t = index_model(&[node("b", ""), node("a", "db1"), late], &[]).expect("fits");
    assert_eq!(t.node_count(), 2);
    assert_eq!((t.node(0).id, t.node(1).id), ("b", "a"));
    assert_eq!(t.node(1).label, "La");
    assert_eq!(t.node(1).database_id, Some("db1"));
    assert_eq!((t.node_index("a"), t.node_index("zz")), (Some(1), None));
}

#[test]
fn edges_skip_taken_ids_and_dangling_endpoints_without_claiming_the_id() {
    let edges = [
        edge("e1", "a", "b"),
        edge("e2", "a", "ghost"),
        edge("e1", "b", "a"),
        edge("e2", "b", "a"),
    ];
    let t = index_model(&[node("a", ""), node("b", "")], &edges).expect("fits");
    assert_eq!(t.edge_count(), 2);
    assert_eq!((t.edge(0).id, t.edge(0).source), ("e1", "a"));
    assert_eq!((t.edge(1).id, t.edge(1).source), ("e2", "b"));
    assert_eq!((t.edge_index("e2"), t.edge_index("e3")), (Some(1), None));
    assert_eq!(
        t.strings().find("ghost"),
        None,
        "a dropped edge interns nothing"
    );
    let kept = t
        .strings()
        .find("e2")
        .expect("a kept edge's id is interned");
    assert_eq!(t.strings().get(kept), "e2");
}

/// The build resolves an endpoint through the arena slot a string sits at, and the
/// slot table names a node only once a node claims that id: a string interned for
/// another field — a label here — is still no node, so the node that claims it later
/// gets its own dense index (not the slot it happens to sit at) and an edge pointing at
/// a string no node ever claimed is dropped.
#[test]
fn an_endpoint_interned_only_as_another_field_is_not_a_node_yet() {
    let mut labelled = node("a", "");
    labelled.label = "b".into();
    let mut dangling = node("z", "");
    dangling.label = "ghost".into();
    let edges = [
        edge("e", "a", "b"),
        edge("d", "a", "ghost"),
        edge("s", "a", "b"),
    ];
    let t = index_model(&[labelled, node("b", ""), dangling], &edges).expect("fits");
    assert_eq!(
        t.node_index("b"),
        Some(1),
        "the slot is not the dense index"
    );
    assert_eq!(
        (t.node_index("ghost"), t.node_index("nope")),
        (None, None),
        "an interned label names no node"
    );
    assert_eq!(t.edge_count(), 2, "e and s; d is dangling");
    assert_eq!((t.edge(0).source, t.edge(0).target), ("a", "b"));
    assert_eq!(t.edge(1).source, "a");
    assert_eq!((t.node_index("a"), t.node_index("z")), (Some(0), Some(2)));
    assert_eq!(
        t.strings().find("d"),
        None,
        "a dropped edge interns nothing"
    );
}

#[test]
fn incident_lists_edges_in_edge_order_and_a_self_loop_twice() {
    let edges = [
        edge("e0", "b", "a"),
        edge("e1", "a", "a"),
        edge("e2", "c", "b"),
        edge("e3", "a", "c"),
    ];
    let nodes = [node("a", ""), node("b", ""), node("c", "")];
    let t = index_model(&nodes, &edges).expect("fits");
    assert_eq!(ids(&t, t.incident(0)), ["e0", "e1", "e1", "e3"]);
    assert_eq!(ids(&t, t.incident(1)), ["e0", "e2"]);
    assert_eq!(ids(&t, t.incident(2)), ["e2", "e3"]);
    assert_eq!(t.nodes().degree, [4, 2, 2]);
    assert_eq!(
        (t.out().row(0), t.inbound().row(0)),
        (&[1, 3][..], &[0, 1][..])
    );
}

/// A→B `child_of` says A is B's child: the edge is filed under B, the wire's endpoints
/// stay as they arrived, and only `parent`/`child` read the flag.
#[test]
fn a_child_of_edge_is_filed_under_its_target_and_keeps_its_endpoints() {
    let mut child_of = edge("h", "a", "b");
    (child_of.kind, child_of.child_first) = (EdgeKind::Hierarchy, true);
    let mut parent_of = edge("p", "a", "b");
    parent_of.kind = EdgeKind::Hierarchy;
    let plain = edge("r", "a", "b");
    let t = index_model(
        &[node("a", ""), node("b", "")],
        &[plain, child_of, parent_of],
    )
    .expect("fits");
    assert_eq!(
        (t.hierarchy().row(0), t.hierarchy().row(1)),
        (&[2][..], &[1][..])
    );
    assert_eq!((t.parent(1), t.child(1)), (1, 0), "child_of: b parents a");
    assert_eq!((t.parent(2), t.child(2)), (0, 1), "parent_of: a parents b");
    assert_eq!((t.edge(1).source, t.edge(1).target), ("a", "b"));
    assert!(t.edge(1).child_first && !t.edge(2).child_first);
    assert_eq!(t.edges().child_first, [false, true, false]);
    assert_eq!(t.hierarchy().rows(), 2);
}

#[test]
fn by_database_is_first_seen_and_stats_count_kept_things() {
    let nodes = [
        node("a", "db2"),
        kind("n", NodeKind::Note),
        node("b", "db1"),
        node("c", "db2"),
        kind("m", NodeKind::Note),
    ];
    let t = index_model(&nodes, &[edge("e", "a", "b")]).expect("fits");
    let groups: Vec<_> = t.by_database().collect();
    assert_eq!(groups, [("db2", &[0, 3][..]), ("db1", &[2][..])]);
    let want = Stats {
        nodes: 5,
        edges: 1,
        databases: 2,
        notes: 2,
    };
    assert_eq!(t.stats(), want);
    assert_eq!(empty_model().stats(), Stats::default());
}

#[test]
fn group_is_the_first_seen_index_of_source_past_255() {
    let nodes: Vec<_> = (0..300)
        .map(|i| NodeRecord {
            source: format!("s{}", i % 290),
            ..node(&i.to_string(), "")
        })
        .collect();
    let t = index_model(&nodes, &[]).expect("fits");
    assert_eq!(t.nodes().group[256], 256, "no u8 aliasing (H9)");
    assert_eq!(t.nodes().group[289], 289);
    assert_eq!(t.nodes().group[290], 0);
}

#[test]
fn nodes_equal_ignores_id_and_compares_floats_like_js_strict_equality() {
    let a = node("a", "db");
    let mut b = NodeRecord {
        id: "b".into(),
        ..a.clone()
    };
    assert!(nodes_equal(&a.view(), &b.view()));
    b.weight = -0.0;
    let mut zero = node("a", "db");
    zero.weight = 0.0;
    assert!(nodes_equal(&zero.view(), &b.view()), "-0 === 0");
    let mut nan = node("a", "db");
    nan.weight = f64::NAN;
    assert!(!nodes_equal(&nan.view(), &nan.view()), "NaN !== NaN");
}

#[test]
fn nodes_equal_sees_every_compared_field() {
    let base = node("a", "db");
    let edits: [fn(&mut NodeRecord); 9] = [
        |n| n.kind = NodeKind::Tag,
        |n| n.database_id = None,
        |n| n.source = "mongo".into(),
        |n| n.label = "x".into(),
        |n| n.group = Some("g".into()),
        |n| n.weight = 0.25,
        |n| n.version = 1.0,
        |n| n.has_note = true,
        |n| n.icon = Some("icon:map".into()),
    ];
    for (i, edit) in edits.iter().enumerate() {
        let mut changed = base.clone();
        edit(&mut changed);
        assert!(!nodes_equal(&base.view(), &changed.view()), "edit {i}");
    }
}

#[test]
fn the_last_u32_is_never_handed_out_as_an_index() {
    let last = u32::MAX as usize;
    assert_eq!(next_index(0, "node index"), Ok(0));
    assert_eq!(next_index(last - 1, "node index"), Ok(u32::MAX - 1));
    let refused = CapacityError { what: "edge index" };
    assert_eq!(next_index(last, "edge index"), Err(refused));
    if let Some(beyond) = last.checked_add(1) {
        assert_eq!(next_index(beyond, "edge index"), Err(refused));
    }
}

/// The columns are reserved once, for the input's length, before anything is pushed: a
/// build never pays for growth slack (the memory measurement depends on it). Grown by
/// pushes instead, 5 nodes would leave room for 8 and 3 edges room for 4.
#[test]
fn index_model_reserves_each_column_once_for_its_input() {
    let nodes: Vec<_> = ["a", "b", "c", "d", "e"].map(|id| node(id, "")).into();
    let edges = [
        edge("e1", "a", "b"),
        edge("e2", "b", "c"),
        edge("e3", "c", "d"),
    ];
    let t = index_model(&nodes, &edges).expect("fits");
    assert_eq!(
        (t.nodes().id.capacity(), t.nodes().weight.capacity()),
        (5, 5)
    );
    assert_eq!(
        (t.edges().id.capacity(), t.edges().source.capacity()),
        (3, 3)
    );
}
