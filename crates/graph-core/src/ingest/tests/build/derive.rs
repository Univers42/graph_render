use super::*;

#[test]
fn one_record_derives_one_node_with_every_role_filled_in() {
    let graph = derived();
    assert_eq!(graph.nodes.len(), 3, "one record and two tag hubs");
    let n = &graph.nodes[0];
    assert_eq!(n.id, "rows:task:r1");
    assert_eq!(n.kind, NodeKind::Record);
    assert_eq!(n.database_id.as_deref(), Some("task"));
    assert_eq!(n.source, "rows");
    assert_eq!(n.label, "Write");
    assert_eq!(n.group.as_deref(), Some("n"));
    assert_eq!(n.weight, 2.0);
    assert_eq!(n.version, 1_700_000_000.0);
    assert!(!n.has_note);
    assert_eq!(n.icon, None);
}

#[test]
fn the_whole_derivation_is_pinned_line_for_line() {
    // The phase's proof in one string: every node and every edge, in derived order, with
    // every field the derivation sets. A changed role, default, kind or strength shows
    // up as a changed line.
    assert_eq!(
        describe(&derived()),
        concat!(
            "node rows:task:r1 Record label=\"Write\" group=Some(\"n\") weight=2 version=1700000000\n",
            "node tag:wip Tag label=\"wip\" group=None weight=0.5 version=0\n",
            "node tag:graph Tag label=\"graph\" group=None weight=0.5 version=0\n",
            "edge rows:task:r0--rows:task:r1:hierarchy: rows:task:r0 -> rows:task:r1 Hierarchy label=\"\" strength=2 directed=false\n",
            "edge rows:task:r1->rows:task:r2:relation:Blocks rows:task:r1 -> rows:task:r2 Relation label=\"Blocks\" strength=1 directed=true\n",
            "edge rows:task:r1--tag:wip:tag:wip rows:task:r1 -> tag:wip Tag label=\"wip\" strength=0.75 directed=false\n",
            "edge rows:task:r1--tag:graph:tag:graph rows:task:r1 -> tag:graph Tag label=\"graph\" strength=0.75 directed=false\n",
        )
    );
}

#[test]
fn every_derived_edge_id_is_the_grammars_own() {
    let graph = derived();
    let ids: Vec<&str> = graph.edges.iter().map(|e| e.id.as_str()).collect();
    // The relation edge's label is the field's *name*, and a directed edge keeps its
    // orientation in its id (`->`), so the two facts the contract declares — the
    // field's human name and the link's `symmetric` — are both visible in the id.
    assert_eq!(
        ids,
        [
            "rows:task:r0--rows:task:r1:hierarchy:",
            "rows:task:r1->rows:task:r2:relation:Blocks",
            "rows:task:r1--tag:wip:tag:wip",
            "rows:task:r1--tag:graph:tag:graph",
        ]
    );
}

#[test]
fn edges_come_out_in_a_fixed_order_hierarchy_then_relations_then_tags() {
    let kinds: Vec<EdgeKind> = derived().edges.iter().map(|e| e.kind).collect();
    assert_eq!(
        kinds,
        [
            EdgeKind::Hierarchy,
            EdgeKind::Relation,
            EdgeKind::Tag,
            EdgeKind::Tag
        ]
    );
}

#[test]
fn every_derived_strength_is_the_tables_own() {
    for edge in derived().edges {
        assert_eq!(edge.strength, edge_strength(edge.kind), "{}", edge.id);
    }
}
