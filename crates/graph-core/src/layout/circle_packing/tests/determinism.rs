//! Two runs of the same topology agree byte for byte, on both paths: nothing in this
//! module reaches for a clock, an RNG, or a hash-order iteration (`fallback`'s own module
//! doc names the two RNG spots SciGraphs has and what replaces them). Also: no `NaN` or
//! `Inf` reaches the wire, on a sweep of seeded synthetic topologies and the shared
//! hierarchy fixtures — this layout does not read the hierarchy, but it still runs over
//! any topology, messy ones included.

use super::support::{complete_graph, topology, wheel};
use crate::layout::circle_packing::run;
use crate::layout::hierarchy::fixture;
use crate::stage::{gate_node_count, seeded_model};
use crate::weights::REFERENCE_DEGREE;
use graph_contract::geometry::NodeGeometry;

fn assert_finite(nodes: &NodeGeometry) {
    let NodeGeometry::Circle { x, y, r } = nodes else {
        panic!("circle packing always emits Circle geometry");
    };
    for value in x.iter().chain(y).chain(r) {
        assert!(value.is_finite(), "{value}");
    }
    assert!(r.iter().all(|&v| v >= 0.0), "no negative radius");
}

#[test]
fn the_exact_and_fallback_paths_are_both_pure() {
    let planar = topology(wheel(6).0, &wheel(6).1);
    let non_planar = topology(5, &complete_graph(5));
    for t in [&planar, &non_planar] {
        let a = run(t).expect("runs");
        let b = run(t).expect("runs");
        assert_eq!(a, b, "same topology, same geometry, twice");
    }
}

/// Small enough that the fallback's own `O(n^2)` passes stay fast, big enough (with
/// `REFERENCE_DEGREE`'s edge density) to be non-planar almost surely.
const DETERMINISM_SEEDS: [u32; 3] = [7, 23, 41];

#[test]
fn seeded_synthetic_topologies_pack_deterministically_and_stay_finite() {
    for seed in DETERMINISM_SEEDS {
        let n = gate_node_count(seed).min(40);
        let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
        let t = crate::index::index_model(&nodes, &edges).expect("fits");
        let a = run(&t).expect("runs");
        let b = run(&t).expect("runs");
        assert_eq!(a, b, "seed {seed}: same topology, same geometry, twice");
        assert_finite(&a.nodes);
    }
}

#[test]
fn every_hierarchy_fixture_still_packs_finitely_as_a_plain_graph() {
    for &(name, _) in fixture::FIXTURES.iter() {
        let (nodes, edges) = fixture::load(name);
        let t = crate::index::index_model(&nodes, &edges).expect("fits");
        let geometry = run(&t).expect("runs");
        assert_finite(&geometry.nodes);
    }
}
