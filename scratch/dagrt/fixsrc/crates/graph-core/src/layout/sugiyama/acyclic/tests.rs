use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

fn topology(nodes: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
    index_model(&n, &e).expect("fits")
}

#[test]
fn a_chain_reverses_nothing() {
    let t = topology(&["a", "b", "c"], &[("ab", "a", "b"), ("bc", "b", "c")]);
    let acyclic = Acyclic::of(&t);
    assert_eq!(acyclic.reversed, [false, false]);
    assert!(acyclic.notes.is_empty());
    assert_eq!(acyclic.arc(&t, 0), (0, 1));
    assert_eq!(acyclic.arc(&t, 1), (1, 2));
}

#[test]
fn a_cycle_is_broken_by_reversing_one_edge_and_noted() {
    // a -> b -> c -> a: a 3-cycle, every node in/out degree 1, tied. `Acyclic::of` takes
    // `ArcOrder::NodeIndex` — the reference's undirected branch, `list(G.nodes())` — so the
    // cycle is broken at its one backwards edge c->a and nothing else moves.
    let t = topology(
        &["a", "b", "c"],
        &[("ab", "a", "b"), ("bc", "b", "c"), ("ca", "c", "a")],
    );
    let acyclic = Acyclic::of(&t);
    assert_eq!(acyclic.reversed, [false, false, true], "c->a reversed");
    assert_eq!(
        acyclic.notes,
        [Note {
            code: NoteCode::EdgeReversed,
            index: 2
        }]
    );
    // Every arc now points from a lower rank to a higher one, and repeating the
    // build gives the same result: determinism.
    for e in 0..3 {
        let (tail, head) = acyclic.arc(&t, e);
        assert!(
            acyclic.rank[tail as usize] < acyclic.rank[head as usize],
            "e{e}"
        );
    }
    assert_eq!(Acyclic::of(&t), acyclic);
}

#[test]
fn the_greedy_feedback_order_is_the_other_branch_and_reverses_more() {
    // `ArcOrder::Feedback` is the reference's `G.is_directed()` branch, which
    // `_acyclic_arcs` never reaches because `common.py:238` builds an `nx.Graph`. It is
    // kept and pinned because it is the port of `_greedy_fas_order` and the tie-break it
    // documents is the one D4 requires — but it is not what the pipeline runs, and this
    // is the test that says so: on `b -> a` the greedy peel makes `b` a source and leaves
    // the edge forward, where `NodeIndex` reverses it.
    let t = topology(&["a", "b"], &[("ba", "b", "a")]);
    assert_eq!(Acyclic::of(&t).arc(&t, 0), (0, 1), "node order reverses it");
    let greedy = Acyclic::oriented(&t, ArcOrder::Feedback);
    assert_eq!(greedy.arc(&t, 0), (1, 0), "the greedy order leaves it");
    assert_eq!(greedy.reversed, [false]);
}

#[test]
fn a_self_loop_is_excluded_and_never_reversed() {
    let t = topology(&["a"], &[("aa", "a", "a")]);
    let acyclic = Acyclic::of(&t);
    assert_eq!(acyclic.reversed, [false]);
    assert!(acyclic.notes.is_empty());
}

#[test]
fn an_arc_owns_exactly_its_own_edges_even_when_they_are_not_adjacent() {
    // `b -> f` twice, at edge 0 and edge 5: one arc, two members, and every edge index in
    // between belongs to some other pair. A member list spelled as a `Range<u32>` over edge
    // indices cannot say that — it claims the whole `0..6` — and `layering.rs` then routes
    // every edge in it through this arc's chain. Seed 66's arc `(20, 28)`, members 38 and
    // 98, range `38..99`: see `docs/measurements/fix-dag-roundtrip.md`.
    let t = topology(
        &["a", "b", "c", "d", "e", "f", "g", "h"],
        &[
            ("bf", "b", "f"),
            ("ac", "a", "c"),
            ("ad", "a", "d"),
            ("af", "a", "f"),
            ("ha", "h", "a"),
            ("bf2", "b", "f"),
            ("ce", "c", "e"),
            ("eh", "e", "h"),
        ],
    );
    let acyclic = Acyclic::of(&t);
    let arcs = Arcs::new(&t, &acyclic);
    let list = arcs.grouped();
    for &(tail, head, ref span) in &list.arcs {
        let at = span.start as usize..span.end as usize;
        assert!(
            at.end <= list.members.len(),
            "arc ({tail}, {head}) over-reads"
        );
        for &e in &list.members[at] {
            assert_eq!(
                arcs.tail_head(e),
                (tail, head),
                "edge {e} is not in arc ({tail}, {head}): it belongs elsewhere"
            );
        }
    }
    let mut routed: Vec<u32> = list.members.clone();
    routed.sort_unstable();
    assert_eq!(routed, [0, 1, 2, 3, 4, 5, 6, 7], "every edge routed once");
    assert_eq!(list.edges, 8, "self-loops aside");
}

#[test]
fn an_empty_topology_produces_empty_acyclic_state() {
    let t = index_model(&[], &[]).expect("fits");
    let acyclic = Acyclic::of(&t);
    assert!(acyclic.rank.is_empty() && acyclic.reversed.is_empty());
}
