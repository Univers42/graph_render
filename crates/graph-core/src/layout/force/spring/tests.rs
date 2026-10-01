//! `layout.force.spring`: the closed cases it has an exact answer for, and the three
//! invariants every run must hold.
//!
//! **The small cases are exact, not close.** A force layout has no coordinate to compare
//! against networkx — the two arms start from different positions (departure 1 in the
//! module doc) and diverge inside the first step — so these tests do not compare
//! coordinates to the library at all. They assert what the *algorithm* determines:
//!
//! - **one node** and **an empty graph**: `spring_layout` returns `center` and `{}`
//!   before computing any force (`layout.py:618-624`), so `(0, 0)` is the whole answer.
//! - **two nodes**: the repulsion is antisymmetric and the attraction is not present for a
//!   single edge pair, so the two nodes stay mirror images about the origin, and the
//!   rescale then puts exactly one of them at `±scale` on the larger axis. Direction
//!   follows the start, which the seed pins.
//! - **a path** and **a star**: the layout is a *function* of the pinned seed, so the
//!   answer is exact and reproducible run to run; the invariants below (finite, distinct,
//!   the path's hop order respected) are what a port bug breaks, and they are checked
//!   against a *property*, never against a stored coordinate.
//!
//! The differential over 1000 gate seeds is `harness/oracle-spring.py` and gates a stress
//! **ratio**; nothing here substitutes for it, and nothing here pretends to.

use super::{ID, Spring, SpringParams};
use crate::layout::coords::probe::{graph, points};
use crate::stage::Stage;
use std::collections::HashSet;

fn run(t: &crate::index::Topology) -> Vec<(f32, f32)> {
    points(&Spring::run(t, &SpringParams::default()).expect("runs"))
}

fn run_with(t: &crate::index::Topology, params: SpringParams) -> Vec<(f32, f32)> {
    points(&Spring::run(t, &params).expect("runs"))
}

#[test]
fn the_id_names_this_layout() {
    assert_eq!(ID, "layout.force.spring");
}

#[test]
fn an_empty_graph_has_no_points() {
    assert_eq!(run(&graph(0, &[])), vec![]);
}

/// `layout.py:620-624`: a single node never enters the force loop, so it is the centre
/// whatever the parameters are.
#[test]
fn one_node_is_the_centre_and_takes_part_in_no_force() {
    let t = graph(1, &[]);
    assert_eq!(run(&t), vec![(0.0, 0.0)]);
    assert_eq!(
        run_with(
            &t,
            SpringParams {
                iterations: 0,
                ..SpringParams::default()
            }
        ),
        vec![(0.0, 0.0)],
        "no iterations is still the centre: the early return is before the loop"
    );
}

/// The two-node case the differential asserts exactly, because here the answer is closed:
/// one edge, so the attraction cancels into the mirror and the pair stays symmetric.
#[test]
fn two_nodes_stay_mirror_images_about_the_origin() {
    let got = run(&graph(2, &[(0, 1)]));
    assert_eq!(got.len(), 2);
    for (i, &(x, y)) in got.iter().enumerate() {
        let (ox, oy) = got[1 - i];
        assert!(
            (x + ox).abs() < 1e-4 && (y + oy).abs() < 1e-4,
            "not mirror images: {got:?}"
        );
    }
}

/// The rescale contract of `layout.py:646`: the largest magnitude over both axes is
/// exactly `scale`, and the centroid is the origin. True of every run, at any `n`.
#[test]
fn every_run_is_centred_and_spans_exactly_the_scale() {
    for n in [2u32, 3, 8, 17] {
        let got = run(&graph(n, &path_edges(n)));
        let mean_x = got.iter().map(|p| f64::from(p.0)).sum::<f64>() / n as f64;
        let mean_y = got.iter().map(|p| f64::from(p.1)).sum::<f64>() / n as f64;
        assert!(mean_x.abs() < 1e-3 && mean_y.abs() < 1e-3, "n={n} {got:?}");
        let extent = got
            .iter()
            .map(|p| f64::from(p.0).abs().max(f64::from(p.1).abs()))
            .fold(0.0, f64::max);
        assert!(
            (extent - 5.0).abs() < 1e-3,
            "n={n} spans {extent}, not the default scale 5.0"
        );
    }
}

/// A path's FR picture respects the path order: node `j` is strictly nearer node 0 than
/// node `j + 1` is, for every `j`. A port that dropped the attraction term, or seeded every
/// node at the same point, breaks this; the ratio metric alone would not.
#[test]
fn a_path_is_drawn_in_hop_order_from_its_first_node() {
    let n = 12;
    let got = run(&graph(n, &path_edges(n)));
    let from_zero = |v: usize| f64::from(got[v].0 - got[0].0).hypot(f64::from(got[v].1 - got[0].1));
    for j in 1..n as usize - 1 {
        assert!(
            from_zero(j) < from_zero(j + 1),
            "node {j} ({}) is not nearer node 0 than node {} ({}): {got:?}",
            from_zero(j),
            j + 1,
            from_zero(j + 1)
        );
    }
}

/// A star's leaves are all at the same radius from the hub, up to the reference's own
/// `f32` narrowing: the hub is fixed by the balance of a symmetric force field and every
/// leaf sees the same repulsion. A port that lost the hub's special position would not.
#[test]
fn a_star_draws_its_leaves_on_one_ring_about_the_hub() {
    let n = 9u32;
    let mut edges = Vec::new();
    for leaf in 1..n {
        edges.push((0, leaf));
    }
    let got = run(&graph(n, &edges));
    let radii: Vec<f64> = got[1..]
        .iter()
        .map(|p| f64::from(p.0 - got[0].0).hypot(f64::from(p.1 - got[0].1)))
        .collect();
    let first = radii[0];
    assert!(
        radii.iter().all(|r| (r - first).abs() < 0.05),
        "leaves are not equidistant: {radii:?}"
    );
    assert!(first > 0.5, "the leaves collapsed onto the hub: {radii:?}");
}

/// No two nodes may coincide: the reference's 0.01 clip keeps a pair apart in the force,
/// and after the rescale two coincident nodes would still coincide. This is the invariant
/// the clip exists for, checked on the output rather than inside the kernel.
#[test]
fn no_two_nodes_coincide_on_any_of_the_gate_shapes() {
    for (n, edges) in [
        (2u32, vec![(0, 1)]),
        (9, (1..9).map(|i| (0, i)).collect()),
        (12, path_edges(12)),
        (30, path_edges(30)),
    ] {
        let got = run(&graph(n, &edges));
        let unique: HashSet<(u32, u32)> =
            got.iter().map(|p| (p.0.to_bits(), p.1.to_bits())).collect();
        assert_eq!(
            unique.len(),
            got.len(),
            "n={n} has coincident nodes: {got:?}"
        );
        assert!(
            got.iter().all(|p| p.0.is_finite() && p.1.is_finite()),
            "n={n} produced a non-finite coordinate"
        );
    }
}

/// The stage's own determinism, the property the 4-way hash gate exists to check across
/// targets: the same topology at the same parameters is bit-identical run to run, and
/// across the parameter struct's other field.
#[test]
fn the_same_topology_and_parameters_are_bit_identical_twice_over() {
    let t = graph(14, &path_edges(14));
    let once = Spring::run(&t, &SpringParams::default()).expect("runs");
    let twice = Spring::run(&t, &SpringParams::default()).expect("runs");
    assert_eq!(points(&once), points(&twice));
    let again = Spring::run(&t, &SpringParams::default()).expect("runs");
    assert_eq!(
        (again.nodes.clone(), again.edges.clone()),
        (once.nodes, once.edges),
        "the geometry itself, not only its point columns"
    );
}

/// `scale` is a real parameter and moves the extent exactly, so the rescale is reached by
/// a caller and not only by the default.
#[test]
fn the_scale_parameter_sets_the_extent() {
    let t = graph(10, &path_edges(10));
    for scale in [1.0f32, 2.5, 17.0] {
        let got = run_with(
            &t,
            SpringParams {
                scale: f64::from(scale),
                ..SpringParams::default()
            },
        );
        let extent = got
            .iter()
            .map(|p| f64::from(p.0).abs().max(f64::from(p.1).abs()))
            .fold(0.0, f64::max);
        assert!((extent - f64::from(scale)).abs() < 1e-3, "{got:?}");
    }
}

/// The iteration count is a real parameter and the early exit is real: zero iterations
/// returns the start field rescaled, which is finite and centred but *not* a settled
/// layout — the property that a knob perturbing `iterations` moves the stage.
#[test]
fn the_iteration_parameter_moves_the_layout() {
    let t = graph(12, &path_edges(12));
    let none = run_with(
        &t,
        SpringParams {
            iterations: 0,
            ..SpringParams::default()
        },
    );
    let some = run_with(&t, SpringParams::default());
    assert_ne!(none, some, "iterations is not reaching the kernel");
    assert!(
        none.iter().all(|p| p.0.is_finite()),
        "an unsettled layout is still finite"
    );
}

/// A graph with no edges at all: the attraction term is empty and the layout is pure
/// repulsion, which still terminates, stays finite and spreads the nodes.
#[test]
fn an_edgeless_graph_is_pure_repulsion_and_still_spreads() {
    let got = run(&graph(6, &[]));
    assert_eq!(got.len(), 6);
    assert!(got.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
    let unique: HashSet<(u32, u32)> = got.iter().map(|p| (p.0.to_bits(), p.1.to_bits())).collect();
    assert_eq!(
        unique.len(),
        6,
        "an edgeless graph must not pile up: {got:?}"
    );
}

/// A self-loop and a parallel edge are the same simple graph to the port, so the answer
/// is the simple path's — the collapse `simple_graph` performs, asserted from the outside.
#[test]
fn a_self_loop_and_a_repeated_pair_are_the_simple_graph() {
    let clean = run(&graph(4, &path_edges(4)));
    let noisy = run(&graph(4, &[(0, 0), (0, 1), (0, 1), (1, 2), (2, 3)]));
    assert_eq!(noisy, clean);
}

/// `path_edges(n)`: the `n`-node path `0-1-...-n-1`, the tree the port's own module doc
/// names as an analytically-determined case.
fn path_edges(n: u32) -> Vec<(u32, u32)> {
    (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
}
