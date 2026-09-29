//! Incremental diff between two topologies (`src/core/model/diff.ts`): set difference
//! over the stable ids, O(n + m). Dense indices are per-topology, so ids are matched by
//! their strings, and the patch names nodes and edges by index into the side they
//! come from.

use crate::index::{Topology, nodes_equal};
use crate::records::EdgeView;

/// `GraphPatch` (`types.ts:93-100`), as dense indices, each list in the oracle's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Patch {
    /// Nodes of `next` whose id `previous` lacks, in `next` order.
    pub added_nodes: Vec<u32>,
    /// Nodes of `next` whose fields changed, in `next` order.
    pub updated_nodes: Vec<u32>,
    /// Nodes of `previous` whose id `next` lacks, in `previous` order.
    pub removed_nodes: Vec<u32>,
    /// Edges of `next` whose id `previous` lacks.
    pub added_edges: Vec<u32>,
    /// Edges of `next` whose fields changed.
    pub updated_edges: Vec<u32>,
    /// Edges of `previous` whose id `next` lacks.
    pub removed_edges: Vec<u32>,
}

/// `edgesEqual` (`diff.ts:31-41`): every field but `id`, `strength` compared with IEEE
/// `==` as JS `===` compares it.
///
/// Ponytail: orientation-blind, for oracle parity — the oracle's edge has no
/// `child_first`, so it is not compared. Failing input: the same edge id switched
/// between `parent_of` and `child_of` with the same endpoints and label. Direction: the
/// patch under-reports (no update listed) while the hierarchy the layouts read flips.
/// Escape hatch: a host that re-types a hierarchy edge rebuilds the topology rather
/// than patching it, or compares `EdgeView::child_first` itself.
pub fn edges_equal(a: &EdgeView<'_>, b: &EdgeView<'_>) -> bool {
    a.source == b.source
        && a.target == b.target
        && a.kind == b.kind
        && a.label == b.label
        && a.strength == b.strength
        && a.directed == b.directed
        && a.record_id == b.record_id
}

/// `diffGraph` (`diff.ts:43-71`).
pub fn diff_graph(previous: &Topology, next: &Topology) -> Patch {
    let mut patch = Patch::default();
    for i in 0..next.node_count() {
        let node = next.node(i);
        match previous.node_index(node.id) {
            None => patch.added_nodes.push(i),
            Some(p) if !nodes_equal(&previous.node(p), &node) => patch.updated_nodes.push(i),
            Some(_) => {}
        }
    }
    patch.removed_nodes = (0..previous.node_count())
        .filter(|&p| next.node_index(previous.node(p).id).is_none())
        .collect();
    for i in 0..next.edge_count() {
        let edge = next.edge(i);
        match previous.edge_index(edge.id) {
            None => patch.added_edges.push(i),
            Some(p) if !edges_equal(&previous.edge(p), &edge) => patch.updated_edges.push(i),
            Some(_) => {}
        }
    }
    patch.removed_edges = (0..previous.edge_count())
        .filter(|&p| next.edge_index(previous.edge(p).id).is_none())
        .collect();
    patch
}

/// `isEmptyPatch` (`diff.ts:74-83`).
pub fn is_empty_patch(patch: &Patch) -> bool {
    patch.added_nodes.is_empty()
        && patch.updated_nodes.is_empty()
        && patch.removed_nodes.is_empty()
        && patch.added_edges.is_empty()
        && patch.updated_edges.is_empty()
        && patch.removed_edges.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::edgekind::EdgeKind;
    use crate::index::index_model;
    use crate::records::EdgeRecord;
    use crate::records::build::{edge, node};

    #[test]
    fn a_model_diffed_against_itself_is_empty() {
        let t = index_model(&[node("a", ""), node("b", "")], &[edge("e", "a", "b")]).expect("fits");
        let patch = diff_graph(&t, &t);
        assert!(is_empty_patch(&patch), "{patch:?}");
    }

    #[test]
    fn nodes_are_added_updated_and_removed_in_the_oracles_order() {
        let before = [node("a", ""), node("b", ""), node("c", "")];
        let mut changed = node("c", "");
        changed.label = "new".into();
        let after = [node("d", ""), changed, node("a", ""), node("e", "")];
        let (p, n) = (index_model(&before, &[]), index_model(&after, &[]));
        let patch = diff_graph(&p.expect("fits"), &n.expect("fits"));
        assert_eq!(patch.added_nodes, [0, 3]);
        assert_eq!(patch.updated_nodes, [1]);
        assert_eq!(patch.removed_nodes, [1]);
        assert!(!is_empty_patch(&patch));
    }

    #[test]
    fn edges_are_added_updated_and_removed_by_id() {
        let nodes = [node("a", ""), node("b", "")];
        let before = [edge("x", "a", "b"), edge("y", "a", "b")];
        let mut stronger = edge("y", "a", "b");
        stronger.strength = 0.9;
        let after = [stronger, edge("z", "b", "a")];
        let p = index_model(&nodes, &before).expect("fits");
        let n = index_model(&nodes, &after).expect("fits");
        let patch = diff_graph(&p, &n);
        assert_eq!(
            (patch.added_edges, patch.updated_edges, patch.removed_edges),
            (vec![1], vec![0], vec![0])
        );
    }

    /// Oracle parity: the oracle's edge has no orientation, so neither its diff nor its id
    /// sees one — the same edge switched from `parent_of` to `child_of` is no change.
    #[test]
    fn switching_an_edge_between_parent_of_and_child_of_is_no_patch() {
        let nodes = [node("a", ""), node("b", "")];
        let mut parent_of = edge("h", "a", "b");
        parent_of.kind = EdgeKind::Hierarchy;
        let child_of = EdgeRecord {
            child_first: true,
            ..parent_of.clone()
        };
        assert!(edges_equal(&parent_of.view(), &child_of.view()));
        let p = index_model(&nodes, &[parent_of]).expect("fits");
        let n = index_model(&nodes, &[child_of]).expect("fits");
        assert_ne!(p.parent(0), n.parent(0), "the hierarchy does see it");
        assert!(is_empty_patch(&diff_graph(&p, &n)));
    }

    #[test]
    fn each_patch_list_alone_makes_it_non_empty() {
        for field in 0..6 {
            let mut patch = Patch::default();
            let list = [
                &mut patch.added_nodes,
                &mut patch.updated_nodes,
                &mut patch.removed_nodes,
                &mut patch.added_edges,
                &mut patch.updated_edges,
                &mut patch.removed_edges,
            ];
            list.into_iter().nth(field).expect("six lists").push(0);
            assert!(!is_empty_patch(&patch), "list {field}");
        }
        assert!(is_empty_patch(&Patch::default()));
    }

    #[test]
    fn edges_equal_sees_every_compared_field_and_ignores_id() {
        let base = edge("x", "a", "b");
        assert!(edges_equal(&base.view(), &edge("other", "a", "b").view()));
        let edits: [fn(&mut EdgeRecord); 7] = [
            |e| e.source = "c".into(),
            |e| e.target = "c".into(),
            |e| e.kind = EdgeKind::Tag,
            |e| e.label = "l".into(),
            |e| e.strength = 0.25,
            |e| e.directed = true,
            |e| e.record_id = Some("row".into()),
        ];
        for (i, edit) in edits.iter().enumerate() {
            let mut changed = base.clone();
            edit(&mut changed);
            assert!(!edges_equal(&base.view(), &changed.view()), "edit {i}");
        }
        let mut nan = base.clone();
        nan.strength = f64::NAN;
        assert!(!edges_equal(&nan.view(), &nan.view()), "NaN !== NaN");
    }
}
