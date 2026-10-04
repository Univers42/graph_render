use super::energy::{Field, Probe, delta, resting};
use super::{DavidsonHarel, DhParams};
use crate::index::{Topology, empty_model, index_model};
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

fn graph(n: u32, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn points(t: &Topology, params: DhParams) -> (Vec<f32>, Vec<f32>) {
    match DavidsonHarel::run(t, &params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

#[test]
fn a_path_of_three_has_pinned_coordinates() {
    let (x, y) = points(&graph(3, &[(0, 1), (1, 2)]), DhParams::default());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3238695016, 3231728135, 3233664090, 3238695016, 3237007480, 3228339621
        ]
    );
}

#[test]
fn empty_single_and_edgeless_graphs_work() {
    assert!(points(&empty_model(), DhParams::default()).0.is_empty());
    assert_eq!(points(&graph(1, &[]), DhParams::default()).0.len(), 1);
    assert_eq!(points(&graph(4, &[]), DhParams::default()).0.len(), 4);
}

#[test]
fn every_point_stays_on_the_canvas() {
    let t = graph(9, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (5, 6), (7, 8)]);
    let (x, y) = points(&t, DhParams::default());
    let limit = 5.0 * 3.0 + 1e-3;
    assert!(x.iter().chain(&y).all(|v| v.abs() <= limit));
}

#[test]
fn the_seed_changes_the_result_and_a_seed_repeats() {
    let t = graph(5, &[(0, 1), (1, 2), (3, 4)]);
    let a = points(&t, DhParams::default());
    let b = points(
        &t,
        DhParams {
            seed: 4,
            ..DhParams::default()
        },
    );
    assert_ne!(a, b);
    assert_eq!(a, points(&t, DhParams::default()));
}

#[test]
fn fine_tuning_rounds_run_and_stay_finite() {
    let p = DhParams {
        fineiter: 2,
        ..DhParams::default()
    };
    let (x, y) = points(&graph(4, &[(0, 1), (1, 2), (2, 3)]), p);
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn zero_rounds_return_the_random_start() {
    let p = DhParams {
        maxiter: 0,
        ..DhParams::default()
    };
    let (x, _) = points(&graph(3, &[(0, 1)]), p);
    assert_eq!(x.len(), 3);
}

#[test]
fn edge_length_energy_rewards_a_shorter_edge() {
    let pos = [[0.0, 0.0], [10.0, 0.0]];
    let adj = vec![vec![1], vec![0]];
    let edges = [(0, 1)];
    let field = Field {
        pos: &pos,
        adj: &adj,
        edges: &edges,
        half_width: 50.0,
    };
    let mut w = DhParams::default().weights;
    (w.node_dist, w.edge_crossings, w.node_edge_dist) = (0.0, 0.0, 0.0);
    let e = delta(&probe(&field, 0, [0.0, 0.0]), &w, 0, ([0.0, 0.0], [4.0, 0.0]));
    assert_eq!(e, 36.0 - 100.0);
}

/// The probe `try_node` builds for node `v` at `at`: the field plus that position's own
/// crossings, so the tests below read the same cache the layout does.
fn probe<'a>(field: &'a Field<'a>, v: u32, at: [f64; 2]) -> Probe<'a, 'a> {
    // Leaked so the probe's borrow outlives this call: a test's field is a stack local and
    // the recorded values are read only inside the assertion that follows.
    Probe::new(Field { ..*field }, Box::leak(Box::new(resting(field, v, at))))
}

#[test]
fn a_crossing_costs_one_and_parallel_segments_none() {
    let pos = [[0.0, 0.0], [10.0, 0.0], [5.0, -5.0], [5.0, 5.0]];
    let adj = vec![vec![1], vec![0], vec![3], vec![2]];
    let edges = [(0, 1), (2, 3)];
    let field = Field {
        pos: &pos,
        adj: &adj,
        edges: &edges,
        half_width: 50.0,
    };
    let mut w = DhParams::default().weights;
    (w.node_dist, w.edge_lengths, w.node_edge_dist) = (0.0, 0.0, 0.0);
    // old vs new position: same crossing, gained a crossing, lost a crossing
    assert_eq!(delta(&probe(&field, 0, [0.0, 0.0]), &w, 0, ([0.0, 0.0], [0.0, 0.0])), 0.0);
    assert_eq!(delta(&probe(&field, 0, [0.0, 20.0]), &w, 0, ([0.0, 20.0], [0.0, 0.0])), 1.0);
    assert_eq!(delta(&probe(&field, 0, [0.0, 0.0]), &w, 0, ([0.0, 0.0], [20.0, 0.0])), -1.0);
}

/// The cache is the layout's whole crossings term: it must record exactly the flags
/// `delta` used to recompute per candidate, one per non-skipped `(neighbour, edge)` pair.
/// A cache that dropped or reordered one would change every layout's coordinates, so this
/// pins the count on a graph with a self-loop-free crossing, a shared edge and a
/// multi-edge neighbour list, and pins that reading the cache equals recomputing.
#[test]
fn the_crossing_cache_is_one_flag_per_pair_and_agrees_with_recomputing() {
    //0--1 crosses 2--3; 0--1 shares an edge with 1--4; 2 repeats as 2's neighbour.
    let pos = [[0.0, 0.0], [10.0, 0.0], [5.0, -5.0], [5.0, 5.0], [20.0, 0.0]];
    let adj = vec![vec![1], vec![0, 4, 0], vec![3], vec![2], vec![1]];
    let edges = [(0, 1), (1, 4), (2, 3)];
    let field = Field {
        pos: &pos,
        adj: &adj,
        edges: &edges,
        half_width: 50.0,
    };
    let mut w = DhParams::default().weights;
    (w.node_dist, w.edge_lengths, w.node_edge_dist) = (0.0, 0.0, 0.0);
    // Neighbours of 0: just 1 (adj[0] = [1]). Non-skipped edges for (0,1): every edge but
    // (0,1) itself and (1,4), which has an end at the neighbour — (2,3) only. So one flag,
    // and it is 1: 0--1 crosses 2--3 from both endpoints.
    let rest = resting(&field, 0, [0.0, 0.0]);
    assert_eq!(rest.crossings, [1]);
    assert_eq!(delta(&probe(&field, 0, [0.0, 0.0]), &w, 0, ([0.0, 0.0], [0.0, 0.0])), 0.0);
    // Node 1's neighbour list repeats 0 and adds 4, and (0,1) and (1,4) are skipped for
    // both, leaving (2,3) three times over: the cache has one flag per pair, duplicates
    // included, and only the neighbour 0 reaches across 2--3.
    assert_eq!(resting(&field, 1, [10.0, 0.0]).crossings, [1, 0, 1]);
    // The repulsion half is indexed by node, one reciprocal distance each, zero at the node
    // itself: 0's own slot is 0 and every other slot is `1 / d2(at, pos[u])`.
    assert_eq!(rest.repulsion[0], 0.0);
    assert_eq!(rest.repulsion[1], 1.0 / 100.0);
    assert_eq!(rest.repulsion[2], 1.0 / 50.0);
    assert_eq!(resting(&field, 1, [10.0, 0.0]).repulsion[1], 0.0);
    // Moving 1 out of the crossing loses both of the crossings it made through the repeated
    // neighbour 0, so the delta is -2: the cache counts a multi-edge neighbour list twice,
    // as the reduction it replaced did.
    assert_eq!(
        delta(&probe(&field, 1, [10.0, 0.0]), &w, 1, ([10.0, 0.0], [10.0, 40.0])),
        -2.0
    );
}
