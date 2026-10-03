use super::super::crossings_for;
use super::transpose::work as transpose_work;
use super::*;
use crate::index::{Topology, index_model};
use crate::layout::sugiyama::acyclic::{Acyclic, Arcs};
use crate::layout::sugiyama::layering::{DUMMY_BUDGET, Layering, assign_layers};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::synthetic::Mulberry32;

/// `(ordering, num_layers)` for `nodes`/`edges`.
fn ordering(nodes: &[&str], edges: &[(&str, &str, &str)]) -> (Ordering, u32) {
    let (layering, num_layers) = layering(nodes, edges);
    (
        Ordering::build(&layering, num_layers).expect("covers every layer"),
        num_layers,
    )
}

/// The ordering graph and the layer count `layered` derives, for `nodes`/`edges`.
fn layering(nodes: &[&str], edges: &[(&str, &str, &str)]) -> (Layering, u32) {
    let n: Vec<_> = nodes.iter().map(|id| node(id, "")).collect();
    let e: Vec<_> = edges.iter().map(|&(id, s, t)| edge(id, s, t)).collect();
    let t = index_model(&n, &e).expect("fits");
    let acyclic = Acyclic::of(&t);
    let list = Arcs::new(&t, &acyclic).grouped();
    let layer = assign_layers(&list);
    let layering = Layering::build(&list, &layer, DUMMY_BUDGET);
    let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
    (layering, num_layers)
}

/// Two layers of `width` vertices, each layer-0 vertex reaching three layer-1 vertices at
/// random from `seed` 7: the widest layer the sweep can be handed without a 200k fixture,
/// and one with real crossings for the transpose to remove.
fn wide_layers(width: usize) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let mut rnd = Mulberry32::new(7);
    let nodes: Vec<NodeRecord> = (0..2 * width).map(|i| node(&i.to_string(), "")).collect();
    let mut edges = Vec::new();
    for i in 0..width {
        for _ in 0..3 {
            let j = rnd.pick(width);
            edges.push(edge(
                &format!("{i}->{j}"),
                &i.to_string(),
                &(width + j).to_string(),
            ));
        }
    }
    (nodes, edges)
}

/// `wide_layers`' indexed topology.
fn wide_topology(width: usize) -> Topology {
    let (n, e) = wide_layers(width);
    index_model(&n, &e).expect("fits")
}

#[test]
fn a_chain_has_one_vertex_per_layer_and_no_crossings() {
    let (o, num_layers) = ordering(&["a", "b", "c"], &[("ab", "a", "b"), ("bc", "b", "c")]);
    assert_eq!(
        (num_layers, &o.layers),
        (3, &vec![vec![0], vec![1], vec![2]])
    );
    assert_eq!(o.crossings, 0);
}

#[test]
fn a_solvable_crossing_is_uncrossed_and_deterministic() {
    // a,b at layer 0; c,d at layer 1; ad and bc cross under (a,b / c,d) but not under
    // (a,b / d,c) or (b,a / c,d): the sweep must find the 0-crossing order.
    let nodes = ["a", "b", "c", "d"];
    let edges = [("ac", "a", "c"), ("bd", "b", "d"), ("ad", "a", "d")];
    let (o, _) = ordering(&nodes, &edges);
    assert_eq!(o.crossings, 0, "layers: {:?}", o.layers);
    let (again, _) = ordering(&nodes, &edges);
    assert_eq!((o.layers, o.crossings), (again.layers, again.crossings));
}

/// L-02: one median per movable vertex per sweep, not one per comparison. A width-`W`
/// layer costs `O(W)` medians here and `O(W log W)` before, so the bound is `16n`: eight
/// sweeps, at most two median passes each, every vertex movable.
#[test]
fn a_wide_layer_computes_each_median_once_and_not_once_per_comparison() {
    let width = 1_000;
    let t = wide_topology(width);
    let before = median_evaluations();
    let crossings = crossings_for(&t);
    let used = median_evaluations() - before;
    assert!(crossings > 0, "the wide layers have crossings to reduce");
    let n = u64::from(t.node_count());
    assert!(
        used <= 16 * n,
        "{used} medians for {n} vertices over eight sweeps"
    );
}

/// L-03: the transpose fills each neighbour buffer at most once per pair it examines,
/// against the eight it took (`pair_crossings` collected and sorted two `Vec`s per call,
/// four calls per pair). Bound: three fills per examined pair, so a pair must reuse at
/// least one of the four lists it reads.
#[test]
fn transpose_fills_a_neighbour_buffer_once_per_pair_and_not_ten_times() {
    let width = 1_000;
    let t = wide_topology(width);
    let before = transpose_work();
    let _ = crossings_for(&t);
    let used = transpose_work();
    let (fills, pairs) = (used[0] - before[0], used[1] - before[1]);
    assert!(pairs > 0, "the transpose ran");
    assert!(
        fills <= 3 * pairs,
        "{fills} fills over {pairs} examined pairs ({:.2} per pair)",
        fills as f64 / pairs as f64
    );
}

/// L-13: a `num_layers` that does not cover a vertex's own layer used to index past the end
/// of `order` (`init_order`) — the vertex was dropped from every row and `Coords` defaulted
/// it to x = 0. RED: it panicked at `ordering.rs:109`. No public input reaches this: every
/// `Topology` gets its layer count from `max() + 1`, so this tests the seam.
#[test]
fn a_layer_count_below_a_vertices_own_layer_is_refused_rather_than_dropping_it() {
    let (layering, num_layers) = layering(
        &["a", "b", "c"],
        &[("ab", "a", "b"), ("bc", "b", "c"), ("ca", "c", "a")],
    );
    let too_few = num_layers - 1;
    assert!(
        layering.layer_of.iter().any(|&l| l >= too_few),
        "the fixture must have a vertex outside `too_few`, else it proves nothing"
    );
    assert_eq!(
        Ordering::build(&layering, too_few).err(),
        Some(StageError::Param {
            name: "num_layers",
            rule: "at or above every vertex's own layer index",
        })
    );
    assert!(Ordering::build(&layering, num_layers).is_ok());
}
