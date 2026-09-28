use super::*;
use crate::edgekind::{EdgeKind, child_first_from_type};
use crate::index::{Topology, index_model};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;
use graph_contract::notes::NoteCode::{CycleEdgeDropped, ExtraParentDropped};

mod fixtures;

/// A hierarchy edge `source` → `target` of wire type `wire`.
fn tree(id: &str, source: &str, target: &str, wire: &str) -> EdgeRecord {
    EdgeRecord {
        kind: EdgeKind::Hierarchy,
        child_first: child_first_from_type(Some(wire)),
        label: wire.into(),
        ..edge(id, source, target)
    }
}

fn nodes(ids: &[&str]) -> Vec<NodeRecord> {
    ids.iter().map(|id| node(id, "")).collect()
}

fn repair(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> (Topology, Hierarchy) {
    let topology = index_model(nodes, edges).expect("fits");
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    (topology, hierarchy)
}

fn note(code: NoteCode, index: u32) -> Note {
    Note { code, index }
}

#[test]
fn the_lowest_edge_index_keeps_the_parent_even_when_dense_order_disagrees() {
    // Row a (dense 0) holds edge 1 and is walked first; b's edge 0 is the lowest.
    let edges = [
        tree("b-m", "b", "m", "parent_of"),
        tree("a-m", "a", "m", "parent"),
    ];
    let (_, h) = repair(&nodes(&["a", "b", "m"]), &edges);
    assert_eq!((h.parent(2), h.parent_edge(2)), (Some(1), Some(0)));
    assert_eq!(h.notes(), [note(ExtraParentDropped, 1)]);
    assert_eq!(h.roots(), [0, 1]);
}

#[test]
fn self_loops_are_never_parents_and_never_noted() {
    let edges = [
        tree("a-a", "a", "a", "parent_of"),
        tree("p-a", "p", "a", "parent_of"),
        tree("p-p", "p", "p", "child_of"),
    ];
    let (_, h) = repair(&nodes(&["p", "a"]), &edges);
    assert_eq!((h.parent(1), h.parent_edge(1)), (Some(0), Some(1)));
    assert_eq!(h.parent(0), None);
    assert_eq!(h.notes(), []);
    assert_eq!((h.roots(), h.root()), (&[0][..], Some(0)));
}

#[test]
fn a_repeated_parallel_parent_edge_is_an_extra_parent() {
    let edges = [
        tree("p-c", "p", "c", "parent_of"),
        tree("c-p", "c", "p", "child_of"),
        tree("p-c-2", "p", "c", "parent"),
    ];
    let (_, h) = repair(&nodes(&["p", "c"]), &edges);
    assert_eq!(h.parent_edge(1), Some(0));
    let want = [note(ExtraParentDropped, 1), note(ExtraParentDropped, 2)];
    assert_eq!(h.notes(), want);
}

#[test]
fn a_parent_cycle_is_broken_at_its_lowest_dense_index() {
    // c > a > b > c: a (dense 0) is the lowest, so its parent edge (1) goes.
    let edges = [
        tree("b-c", "b", "c", "parent_of"),
        tree("c-a", "c", "a", "parent_of"),
        tree("b-a", "b", "a", "child_of"),
    ];
    let (_, h) = repair(&nodes(&["a", "b", "c"]), &edges);
    assert_eq!(h.notes(), [note(CycleEdgeDropped, 1)]);
    assert_eq!((h.roots(), h.root()), (&[0][..], Some(0)));
    assert_eq!([h.parent(1), h.parent(2)], [Some(0), Some(1)]);
    assert_eq!([h.depth(0), h.depth(1), h.depth(2)], [0, 1, 2]);
    assert_eq!(h.order(), [0, 1, 2]);
}

#[test]
fn one_root_is_the_tree_root_and_two_hang_off_a_virtual_root() {
    let (_, one) = repair(&nodes(&["a"]), &[]);
    assert_eq!((one.root(), one.virtual_root()), (Some(0), None));
    assert_eq!(
        (one.order(), one.depth(0), one.max_depth()),
        (&[0][..], 0, 0)
    );
    // One tree and an isolated node: the jump from a single root to a virtual one.
    let (_, two) = repair(&nodes(&["p", "c", "s"]), &[tree("p-c", "p", "c", "parent")]);
    assert_eq!((two.root(), two.virtual_root()), (Some(3), Some(3)));
    assert_eq!((two.roots(), two.children(3)), (&[0, 2][..], &[0, 2][..]));
    assert_eq!(two.order(), [3, 0, 2, 1]);
    let depths: Vec<_> = (0..=3).map(|v| two.depth(v)).collect();
    assert_eq!((depths, two.max_depth()), (vec![1, 2, 1, 0], 2));
    assert_eq!(two.parent(0), None, "the virtual root is no one's parent");
}

#[test]
fn an_empty_topology_has_no_root() {
    let (_, h) = repair(&[], &[]);
    assert_eq!(
        (h.node_count(), h.root(), h.virtual_root()),
        (0, None, None)
    );
    assert_eq!(
        (h.roots(), h.order(), h.notes()),
        (&[][..], &[][..], &[][..])
    );
    assert_eq!((h.children(0), h.max_depth()), (&[][..], 0));
}

#[test]
fn children_are_in_ascending_dense_index_whatever_the_edge_order() {
    let edges = [
        tree("p-z", "p", "z", "parent_of"),
        tree("x-p", "x", "p", "child_of"),
        tree("p-y", "p", "y", "x_hierarchy_y"),
        edge("rel", "p", "q"),
    ];
    let (_, h) = repair(&nodes(&["p", "x", "y", "z", "q"]), &edges);
    assert_eq!(h.children(0), [1, 2, 3]);
    assert_eq!(h.roots(), [0, 4], "a relation edge is not a parent");
}

/// Every property D-H promises, for one repaired topology.
fn check(t: &Topology, h: &Hierarchy) {
    check_reached(t, h);
    check_forest(t, h);
    check_notes(t, h);
}

/// Every node, the virtual root included, is reached exactly once from the root.
fn check_reached(t: &Topology, h: &Hierarchy) {
    let n = t.node_count();
    let root = h.root().expect("n > 0");
    assert_eq!(h.virtual_root().is_some(), h.roots().len() >= 2);
    let mut seen = vec![0u32; n as usize + 1];
    for &v in h.order() {
        seen[v as usize] += 1;
    }
    let reached = n + u32::from(h.virtual_root().is_some());
    assert_eq!(h.order().len() as u32, reached, "every node reached");
    let once = seen[..reached as usize].iter().all(|&s| s == 1);
    assert!(once, "exactly once");
    assert_eq!((h.order()[0], h.depth(root)), (root, 0));
    for p in 0..=n {
        assert!(h.children(p).windows(2).all(|w| w[0] < w[1]));
    }
}

/// Each kept parent is a real, non-loop hierarchy edge, one level up: a forest.
fn check_forest(t: &Topology, h: &Hierarchy) {
    let top = u32::from(h.virtual_root().is_some());
    for v in 0..t.node_count() {
        let Some(e) = h.parent_edge(v) else {
            assert!(h.roots().contains(&v), "no parent: a root");
            assert_eq!(h.depth(v), top);
            continue;
        };
        assert_eq!(t.edges().kind[e as usize], EdgeKind::Hierarchy);
        assert_eq!((t.child(e), h.parent(v)), (v, Some(t.parent(e))));
        assert_ne!(t.parent(e), v, "a self-loop is never a parent");
        let step = h.depth(t.parent(e)) + 1;
        assert_eq!(h.depth(v), step, "a forest: depth steps by one");
    }
}

/// The notes are canonical and name exactly the hierarchy edges the repair dropped.
fn check_notes(t: &Topology, h: &Hierarchy) {
    assert!(h.notes().windows(2).all(|w| w[0] < w[1]), "ascending");
    let dropped: Vec<u32> = (0..t.edge_count())
        .filter(|&e| t.edges().kind[e as usize] == EdgeKind::Hierarchy)
        .filter(|&e| t.parent(e) != t.child(e) && h.parent_edge(t.child(e)) != Some(e))
        .collect();
    let mut noted: Vec<u32> = h.notes().iter().map(|d| d.index).collect();
    noted.sort();
    assert_eq!(
        noted, dropped,
        "the dropped edges are exactly the recorded ones"
    );
    for cut in h.notes().iter().filter(|d| d.code == CycleEdgeDropped) {
        let root = h.roots().contains(&t.child(cut.index));
        assert!(root, "a cycle breaks at a root");
    }
}

fn seeded(seed: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    seeded_model(seed, 2 + seed * 13 % 400, REFERENCE_DEGREE)
}

#[test]
fn every_seeded_model_repairs_to_a_forest_reached_once_from_the_root() {
    let mut codes = [0u32; 2];
    for seed in 0..64 {
        let (nodes, edges) = seeded(seed);
        let (t, h) = repair(&nodes, &edges);
        check(&t, &h);
        for d in h.notes() {
            codes[d.code.code() as usize - 1] += 1;
        }
    }
    assert!(
        codes.iter().all(|&c| c > 0),
        "both repairs drawn: {codes:?}"
    );
}

#[test]
fn renaming_every_id_changes_nothing_but_the_names() {
    for seed in [3, 17, 40] {
        let (mut nodes, mut edges) = seeded(seed);
        let (_, before) = repair(&nodes, &edges);
        let rename = |id: &mut String| *id = format!("renamed/{}", id.len() * 7 % 5) + id;
        nodes.iter_mut().for_each(|n| rename(&mut n.id));
        for e in &mut edges {
            [&mut e.id, &mut e.source, &mut e.target]
                .into_iter()
                .for_each(rename);
        }
        let (_, after) = repair(&nodes, &edges);
        assert_eq!(before, after, "seed {seed}");
    }
}
