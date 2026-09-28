//! `fixtures/hierarchy/*.json` pinned: the repair, the notes and the tree each gives.

use super::*;
use crate::layout::hierarchy::fixture::{self, load};

/// Fixture `name` indexed and repaired.
fn fixture(name: &str) -> (Topology, Hierarchy) {
    let (nodes, edges) = load(name);
    repair(&nodes, &edges)
}

/// Every node as `(id, parent id, depth)`, sorted by id: what survives a relabelling.
fn by_id(t: &Topology, h: &Hierarchy) -> Vec<(String, Option<String>, u32)> {
    let id = |v: u32| t.node(v).id.to_string();
    let mut rows: Vec<_> = (0..t.node_count())
        .map(|v| (id(v), h.parent(v).map(id), h.depth(v)))
        .collect();
    rows.sort();
    rows
}

/// `(child, parent)` pairs, as ids, written the way the fixture's `about` states them.
fn parents(t: &Topology, h: &Hierarchy) -> Vec<(String, String)> {
    let id = |v: u32| t.node(v).id.to_string();
    (0..t.node_count())
        .filter_map(|v| h.parent(v).map(|p| (id(v), id(p))))
        .collect()
}

fn pairs(text: &str) -> Vec<(String, String)> {
    text.split(", ")
        .map(|pair| pair.split_once('<').expect("child<parent"))
        .map(|(c, p)| (c.to_string(), p.to_string()))
        .collect()
}

#[test]
fn cyclic_pins_every_repair_and_its_notes() {
    let (t, h) = fixture("cyclic");
    let want = [
        note(CycleEdgeDropped, 7),
        note(CycleEdgeDropped, 8),
        note(ExtraParentDropped, 3),
        note(ExtraParentDropped, 4),
    ];
    assert_eq!(h.notes(), want);
    let dropped: Vec<_> = want.iter().map(|n| t.edge(n.index).id).collect();
    assert_eq!(dropped, ["x-z", "c-d", "a-m", "r-a-again"]);
    assert_eq!(parents(&t, &h), pairs("a<r, b<z, m<b, y<x, z<y, d<c, t<c"));
    let kept = [1, 2, 3, 5, 6, 8, 9].map(|v| h.parent_edge(v).expect("kept"));
    assert_eq!(
        kept,
        [2, 11, 1, 5, 6, 9, 10],
        "b-m (1) beats a-m (3): lowest edge index"
    );
    assert_eq!((h.roots(), h.virtual_root()), (&[0, 4, 7][..], Some(10)));
    let depths: Vec<_> = (0..10).map(|v| h.depth(v)).collect();
    assert_eq!(depths, [1, 2, 4, 5, 1, 2, 3, 1, 2, 2]);
    assert_eq!(h.order(), [10, 0, 4, 7, 1, 5, 8, 9, 6, 2, 3]);
    assert_eq!(
        (h.children(10), h.children(7), h.max_depth()),
        (&[0, 4, 7][..], &[8, 9][..], 5)
    );
}

#[test]
fn the_balanced_tree_needs_no_repair_whatever_the_spelling() {
    let (t, h) = fixture("tree-balanced");
    assert_eq!(h.notes(), []);
    assert_eq!(
        (h.roots(), h.root(), h.virtual_root()),
        (&[0][..], Some(0), None)
    );
    let want = "a<r, b<r, a1<a, a2<a, b1<b, b2<b, a1x<a1, a1y<a1, a2x<a2, a2y<a2, \
                b1x<b1, b1y<b1, b2x<b2, b2y<b2";
    assert_eq!(parents(&t, &h), pairs(want));
    assert_eq!(
        h.order(),
        (0..15).collect::<Vec<_>>(),
        "listed breadth first"
    );
    let depths: Vec<_> = (0..15).map(|v| h.depth(v)).collect();
    assert_eq!(depths, [0, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3]);
    assert_eq!(
        (h.children(0), h.children(4), h.max_depth()),
        (&[1, 2][..], &[9, 10][..], 3)
    );
}

#[test]
fn the_degenerate_chain_is_rooted_at_its_last_listed_node() {
    let (t, h) = fixture("tree-degenerate");
    assert_eq!(h.notes(), []);
    assert_eq!((h.roots(), h.root()), (&[7][..], Some(7)));
    assert_eq!(h.order(), [7, 6, 5, 4, 3, 2, 1, 0]);
    let want = pairs("n7<n6, n6<n5, n5<n4, n4<n3, n3<n2, n2<n1, n1<n0");
    assert_eq!(parents(&t, &h), want);
    assert!((0..8).all(|v| h.depth(v) == 7 - v));
    assert_eq!(h.max_depth(), 7);
}

#[test]
fn the_forest_hangs_every_root_off_one_virtual_root() {
    let (t, h) = fixture("forest");
    assert_eq!(h.notes(), []);
    assert_eq!((h.roots(), h.virtual_root()), (&[0, 1, 2, 6][..], Some(8)));
    assert_eq!(h.children(8), [0, 1, 2, 6]);
    assert_eq!(parents(&t, &h), pairs("p1<p, q1<q, p2<p, p11<p1"));
    assert_eq!((h.children(1), h.children(3)), (&[3, 5][..], &[7][..]));
    assert_eq!(h.order(), [8, 0, 1, 2, 6, 4, 3, 5, 7]);
    let depths: Vec<_> = (0..8).map(|v| h.depth(v)).collect();
    assert_eq!((depths, h.max_depth()), (vec![1, 1, 1, 2, 2, 2, 1, 3], 3));
}

#[test]
fn every_fixture_keeps_the_promised_properties() {
    for (name, _) in fixture::FIXTURES {
        let (t, h) = fixture(name);
        check(&t, &h);
    }
}

/// Relabel invariance where it applies: with no cycle and no second parent the repair
/// reads neither the dense order nor the edge order, so reordering either moves no
/// node's parent or depth. (A cycle breaks at the lowest dense index and a second
/// parent loses to the lowest edge index, so `cyclic` is order-bound by decision.)
#[test]
fn reordering_an_acyclic_fixture_moves_no_parent_and_no_depth() {
    for name in ["tree-balanced", "tree-degenerate", "forest"] {
        let (nodes, edges) = load(name);
        let (t, h) = repair(&nodes, &edges);
        let want = by_id(&t, &h);
        let mut rotated = nodes.clone();
        rotated.rotate_left(nodes.len() / 3);
        let reversed: Vec<_> = nodes.iter().rev().cloned().collect();
        let back: Vec<_> = edges.iter().rev().cloned().collect();
        for (n, e) in [(&reversed, &edges), (&rotated, &back), (&nodes, &back)] {
            let (t2, h2) = repair(n, e);
            assert_eq!(by_id(&t2, &h2), want, "{name}");
            assert_eq!(h2.notes(), [], "{name}");
        }
    }
}
