//! `run_scaled` against SciGraphs' own formula (`hierarchical.py:679-685`), read off a
//! three-node chain and off the two branches that formula takes when the drawing has no
//! width and no height.

use super::run_scaled;
use crate::index::Topology;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

/// `layout.dag.sugiyama` at the dispatcher's `scale` (`apply_graph_layout`, `scale=5.0`).
const SCALE: f32 = 5.0;

fn topology(nodes: &[&str], edges: &[(&str, &str, &str)]) -> Topology {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
    index_model(&n, &e).expect("fits")
}

/// Every node's `(x, y)` at `SCALE`, in node order. `z` is not a column here: the judge's
/// `columns` already states it is `0.0` for a planar layout on both arms.
fn points(t: &Topology) -> Vec<[f32; 2]> {
    let geometry = run_scaled(t, SCALE).expect("scale 5.0 is fine");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("run_scaled draws points")
    };
    (0..x.len()).map(|i| [x[i], y[i]]).collect()
}

#[test]
fn a_three_node_chain_is_the_formula_on_a_zero_width_drawing() {
    // layer_of is 0,1,2, so max_layer is 2 and Y is (-scale, 0, +scale). Every vertex sits
    // in slot 0 of its own layer, so X is 0 everywhere and `hi - lo` is 0 — the
    // `(hi - lo) or 1.0` branch, which must give ((0 - 0) / 1 * 2 - 1) * scale = -scale
    // rather than a division by zero.
    let t = topology(&["a", "b", "c"], &[("ab", "a", "b"), ("bc", "b", "c")]);
    assert_eq!(points(&t), [[-5.0, -5.0], [-5.0, 0.0], [-5.0, 5.0]]);
}

#[test]
fn a_fan_out_pins_both_axes_at_the_two_ends_of_the_range() {
    // `a -> b, a -> c`: layer_of is 0,1,1 and max_layer is 1, so Y is -scale on layer 0 and
    // +scale on layer 1. The priority method leaves X at 0,-1,0 (b pulls the fan-out toward
    // it, c is pushed one gap right), so lo = -1, hi = 0, width = 1 and the two extremes
    // land on ±scale with `a` sharing `c`'s.
    let t = topology(&["a", "b", "c"], &[("ab", "a", "b"), ("ac", "a", "c")]);
    assert_eq!(points(&t), [[5.0, -5.0], [-5.0, 5.0], [5.0, 5.0]]);
}

#[test]
fn a_drawing_with_no_layers_is_flat_and_spans_the_full_width() {
    // No edges at all: one layer holding both nodes, so max_layer is 0 and the reference's
    // conditional puts every Y at 0.0 rather than dividing. X is the layer slot index 0,1,
    // so the two nodes still take the two ends of the range.
    let t = topology(&["a", "b"], &[]);
    assert_eq!(points(&t), [[-5.0, 0.0], [5.0, 0.0]]);
}

#[test]
fn an_edge_written_backwards_is_oriented_by_node_order_as_the_reference_orients_it() {
    // SciGraphs builds an **undirected** `nx.Graph` (`common.py:238`), so `_acyclic_arcs`
    // takes its `list(G.nodes())` branch (`hierarchical.py:304`) and every arc is oriented
    // from the lower node index to the higher one — the greedy feedback-arc-set order is
    // only reached for a `nx.DiGraph`, which this pipeline never sees. `single-backwards`
    // is the smallest graph where the two orders disagree: on `1 -> 0` the reference's arcs
    // are `[(0, 1)]`, its layers `[0, 1]`, its order `[[0], [1]]` and its positions
    // `[-5, -5]` and `[-5, 5]`. Read off the reference itself, not derived.
    let t = topology(&["a", "b"], &[("ba", "b", "a")]);
    assert_eq!(points(&t), [[-5.0, -5.0], [-5.0, 5.0]]);
}

#[test]
fn a_scale_that_is_not_finite_and_above_zero_is_refused() {
    let t = topology(&["a", "b"], &[("ab", "a", "b")]);
    for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(run_scaled(&t, scale).is_err(), "scale {scale}");
    }
}
