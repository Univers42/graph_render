//! Unit tests for [`super`].
//!
//! The containment/no-overlap sweep lives in [`invariants`], whose module doc carries the
//! two measured float findings and their tolerances; the d3-hierarchy@3.1.2 golden
//! values live in [`golden`]. This file holds the structural, weight-clamp and
//! edge-case tests.

use super::*;
use crate::edgekind::{EdgeKind, child_first_from_type};
use crate::index::{Topology, index_model};
use crate::layout::hierarchy::fixture;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;

mod golden;
mod invariants;
mod shapes;

/// A hierarchy edge `source` -> `target` of wire type `wire` (mirrors `hierarchy`'s own
/// test helper, private to that module).
fn tree(id: &str, source: &str, target: &str, wire: &str) -> EdgeRecord {
    EdgeRecord {
        kind: EdgeKind::Hierarchy,
        child_first: child_first_from_type(Some(wire)),
        label: wire.into(),
        ..edge(id, source, target)
    }
}

fn weighted(id: &str, weight: f64) -> NodeRecord {
    NodeRecord {
        weight,
        ..node(id, "")
    }
}

fn from_fixture(name: &str) -> Topology {
    let (nodes, edges) = fixture::load(name);
    index_model(&nodes, &edges).expect("fits")
}

/// The `Box` centre/size columns, or a panic: every test here is
/// `layout.treemap.squarified`, `Box` nodes.
fn boxes(g: &Geometry) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    let NodeGeometry::Box { x, y, w, h } = &g.nodes else {
        panic!("box geometry");
    };
    (x.clone(), y.clone(), w.clone(), h.clone())
}

/// The contract's centre/size columns for an `f64` box, cast the way `to_geometry` casts.
fn leaf_box(x0: f64, y0: f64, x1: f64, y1: f64) -> (f32, f32, f32, f32) {
    (
        ((x0 + x1) / 2.0) as f32,
        ((y0 + y1) / 2.0) as f32,
        (x1 - x0) as f32,
        (y1 - y0) as f32,
    )
}

#[test]
fn clamp_weight_only_touches_non_positive_or_non_finite() {
    for bad in [0.0, -5.0, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(clamp_weight(bad), WEIGHT_EPSILON, "{bad}");
    }
    assert_eq!(clamp_weight(0.3), 0.3);
    assert_eq!(
        clamp_weight(1e-7),
        1e-7,
        "small positive is not the epsilon path"
    );
}

#[test]
fn sorted_children_is_descending_value_stable_on_ties() {
    let value = [0.0, 5.0, 9.0, 5.0]; // indices 1, 2, 3 are the candidates below
    assert_eq!(rows::sorted_children(&[1, 2, 3], &value), [2, 1, 3]);
}

/// **d3's comparator on two aggregates that both overflow to `+inf`.** `b.value - a.value`
/// is `inf - inf == NaN`, and `Array.prototype.sort` coerces a `NaN` return to `+0`
/// (ECMA-262, `SortCompare`: "If v is NaN, return +0"), i.e. keep order — while
/// `f64::total_cmp` reports two `+inf` as **equal** and a stable sort therefore keeps order
/// too. The port needs no special case, and this pins that the two really do agree on the
/// overflow rather than on paper.
///
/// The overflow is built, not typed in: `a` and `b` each carry `1e308` own weight *and* a
/// child of `1e308`, so `node_values` sums `1e308 + 1e308 == inf` for both exactly as a
/// real aggregate would, and `c` stays at `5.0`. `js_order` then re-sorts the same slice
/// through d3's comparator with the `NaN -> +0` coercion, and both orders must be
/// `[a, b, c]`: the two infinities keep their ascending dense index, and `5.0` is last.
#[test]
fn two_infinite_aggregates_sort_as_equal_and_js_keeps_their_order_too() {
    let nodes = vec![
        weighted("p", 1e308),
        weighted("a", 1e308),
        weighted("x", 1e308),
        weighted("b", 1e308),
        weighted("y", 1e308),
        weighted("c", 5.0),
    ];
    let edges = vec![
        tree("p-a", "p", "a", "parent_of"),
        tree("a-x", "a", "x", "parent_of"),
        tree("p-b", "p", "b", "parent_of"),
        tree("b-y", "b", "y", "parent_of"),
        tree("p-c", "p", "c", "parent_of"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    let value = rows::node_values(&topology, &hierarchy);
    assert_eq!(value[1], f64::INFINITY, "a: own + child overflows");
    assert_eq!(value[3], f64::INFINITY, "b: own + child overflows");
    assert_eq!(value[5], 5.0);
    let children = hierarchy.children(0);
    assert_eq!(children, [1, 3, 5], "p's children, ascending dense index");
    assert_eq!(rows::sorted_children(children, &value), [1, 3, 5]);
    assert_eq!(js_order(children, &value), vec![1, 3, 5]);
}

/// d3's `children.sort((a, b) => b.value - a.value)` over the same slice, with the `NaN`
/// return coerced to `+0` the way `Array.prototype.sort` coerces it, and its stable sort
/// otherwise unchanged.
fn js_order(children: &[u32], value: &[f64]) -> Vec<u32> {
    let mut sorted = children.to_vec();
    sorted.sort_by(|&a, &b| {
        let difference = value[b as usize] - value[a as usize];
        let coerced = if difference.is_nan() { 0.0 } else { difference };
        coerced
            .partial_cmp(&0.0)
            .expect("a number or the coerced 0.0")
    });
    sorted
}

/// Root `1.0`, leaves `2.0` and `1.0`: the two leaves take the left three quarters of the
/// unit square as stacked y-slices, leaving the root's own quarter empty on the right.
///
/// Split from the arithmetic it checks so each stays under the house line limit. The
/// expected values are recomputed in `squarify.js`'s own operation order rather than
/// hand-typed decimals, which a differently-ordered computation would not reproduce: the
/// parent is the root, whose `value` is `1 + 2 + 1 = 4` over the unit square, so
/// `slice` runs with `k = (y1 - y0) / parent.value = 1/3`.
#[test]
fn a_hand_worked_three_node_tree_leaves_the_roots_own_quarter_empty() {
    let nodes = vec![
        weighted("root", 1.0),
        weighted("c1", 2.0),
        weighted("c2", 1.0),
    ];
    let edges = vec![
        tree("r-c1", "root", "c1", "parent_of"),
        tree("r-c2", "root", "c2", "parent_of"),
    ];
    let topology = index_model(&nodes, &edges).expect("fits");
    let (x, y, w, h) = boxes(&run(&topology).expect("runs"));
    assert_eq!(
        (x[0], y[0], w[0], h[0]),
        (0.5, 0.5, 1.0, 1.0),
        "root is the unit square"
    );
    let k = 1.0_f64 / 3.0_f64;
    let c1_y1 = 2.0_f64 * k;
    let c2_y1 = c1_y1 + 1.0_f64 * k;
    assert_eq!(
        (x[1], y[1], w[1], h[1]),
        leaf_box(0.0, 0.0, 0.75, c1_y1),
        "c1: root's quarter is the gap"
    );
    assert_eq!(
        (x[2], y[2], w[2], h[2]),
        leaf_box(0.0, c1_y1, 0.75, c2_y1),
        "c2 shares c1's column exactly"
    );
}

/// **n = 0**: an empty topology has no node and so no root, which is the one case
/// `compute` short-circuits. `run` must still emit well-formed empty `Box` columns and
/// `Line` edges rather than panicking on the missing root.
#[test]
fn an_empty_topology_has_no_boxes_and_no_notes() {
    let geometry = run(&index_model(&[], &[]).expect("fits")).expect("runs");
    assert_eq!(boxes(&geometry), (vec![], vec![], vec![], vec![]));
    assert_eq!(geometry.notes, []);
    assert_eq!(geometry.edges, EdgeGeometry::Line);
}

/// **n = 1**: a lone node is its own root, gets the whole unit square, and has no children
/// — so `squarify_children` is never entered at all. This pins the `kids.is_empty()` skip
/// and the one-row `Boxes` allocation, which the n = 0 case never reaches.
#[test]
fn a_single_node_is_the_whole_unit_square() {
    let topology = index_model(&[weighted("only", 3.5)], &[]).expect("fits");
    let (x, y, w, h) = boxes(&run(&topology).expect("runs"));
    assert_eq!((x, y, w, h), (vec![0.5], vec![0.5], vec![1.0], vec![1.0]));
    let hierarchy = Hierarchy::of(&topology).expect("fits");
    assert_eq!(hierarchy.children(0), &[] as &[u32], "no children to tile");
}

#[test]
fn two_runs_over_the_same_topology_are_byte_equal() {
    let (nodes, edges) = seeded_model(11, 40, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("fits");
    assert_eq!(run(&topology).expect("runs"), run(&topology).expect("runs"));
}

#[test]
fn no_geometry_is_ever_nan_or_infinite() {
    let mut topologies: Vec<Topology> = ["tree-balanced", "tree-degenerate", "forest", "cyclic"]
        .into_iter()
        .map(from_fixture)
        .collect();
    for seed in 0..32u32 {
        let (nodes, edges) = seeded_model(seed, 3 + seed % 50, REFERENCE_DEGREE);
        topologies.push(index_model(&nodes, &edges).expect("fits"));
    }
    for topology in &topologies {
        let (x, y, w, h) = boxes(&run(topology).expect("runs"));
        let finite = x
            .iter()
            .chain(&y)
            .chain(&w)
            .chain(&h)
            .all(|v| v.is_finite());
        assert!(finite, "non-finite geometry");
    }
}
