use super::*;
use crate::index::index_model;
use crate::layout::sugiyama::acyclic::{Acyclic, Arcs};
use crate::records::build::{edge, node};
/// `(layering, layer_of)` for `nodes`/`edges` at `budget`.
fn built(nodes: &[&str], edges: &[(&str, &str, &str)], budget: u32) -> (Layering, Vec<u32>) {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
    let t = index_model(&n, &e).expect("fits");
    let acyclic = Acyclic::of(&t);
    let list = Arcs::new(&t, &acyclic).grouped();
    let layer = assign_layers(&list);
    (Layering::build(&list, &layer, budget), layer)
}
#[test]
fn a_chain_has_no_dummies_and_a_reversed_cycle_still_spans_forward() {
    let e = [("ab", "a", "b"), ("bc", "b", "c"), ("cd", "c", "d")];
    let (l, layer) = built(&["a", "b", "c", "d"], &e, DUMMY_BUDGET);
    assert_eq!(layer, [0, 1, 2, 3]);
    assert_eq!(l.route, [Route::Direct, Route::Direct, Route::Direct]);
    assert!(l.notes.is_empty());
    // c->a closes a cycle, reversing to a->c: still spans forward, so nothing goes Straight.
    let e2 = [("ab", "a", "b"), ("bc", "b", "c"), ("ca", "c", "a")];
    let (l2, layer2) = built(&["a", "b", "c"], &e2, DUMMY_BUDGET);
    assert!(l2.route.iter().all(|r| !matches!(r, Route::Straight)));
    assert!(layer2.iter().all(|&x| x <= 2));
}
#[test]
fn a_multi_span_edge_gets_one_dummy_per_intermediate_layer() {
    let e = [
        ("ab", "a", "b"),
        ("bc", "b", "c"),
        ("cd", "c", "d"),
        ("ad", "a", "d"),
    ];
    let (l, layer) = built(&["a", "b", "c", "d"], &e, DUMMY_BUDGET);
    assert_eq!(layer, [0, 1, 2, 3]);
    assert_eq!(l.route[3], Route::Chain { first: 4, count: 2 });
    assert_eq!(l.up.len(), 6, "4 real + 2 dummy");
    assert_eq!(l.down[0], [1, 4], "a's direct edge, then its dummy chain");
    assert_eq!((l.up[4][0], l.down[4][0]), (0, 5));
    assert_eq!((l.up[5][0], l.down[5][0]), (4, 3));
}
#[test]
fn a_lowered_budget_leaves_the_longest_spans_straight_and_noted() {
    // Two 2-layer edges (1 dummy each) plus a 4-layer edge (3 dummies): budget 2 admits
    // only the two smallest spans, leaving the longest one straight.
    let e = [
        ("ab", "a", "b"),
        ("bc", "b", "c"),
        ("cd", "c", "d"),
        ("de", "d", "e"),
        ("ac", "a", "c"),
        ("bd", "b", "d"),
        ("ae", "a", "e"),
    ];
    let (l, _) = built(&["a", "b", "c", "d", "e"], &e, 2);
    assert_eq!(l.route[4], Route::Chain { first: 5, count: 1 });
    assert_eq!(l.route[5], Route::Chain { first: 6, count: 1 });
    assert_eq!(l.route[6], Route::Straight, "the 4-layer span");
    assert_eq!(
        l.notes,
        [Note {
            code: NoteCode::DummyBudgetExceeded,
            index: 6
        }]
    );
}
