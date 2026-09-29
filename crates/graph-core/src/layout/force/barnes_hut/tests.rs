//! Split from `barnes_hut.rs` for the house line cap. Includes the devil C9 fixture:
//! a mid-degree node wired to several much higher-degree hubs, the shape the Jacobi
//! reformulation of link (devil C7) is most at risk from, since every hub pulls from a
//! fixed pre-tick snapshot instead of seeing the others' corrections mid-pass.

use super::sim::Sim;
use super::{BarnesHut, TICKS};
use crate::index::index_model;
use crate::layout::force::params::ForceParams;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

fn line(n: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    (nodes, edges)
}

#[test]
fn a_small_graph_settles_to_finite_positions() {
    let (nodes, edges) = line(12);
    let geometry = BarnesHut::run(
        &index_model(&nodes, &edges).expect("fits"),
        &ForceParams::default(),
    )
    .expect("finite");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("force layout is point geometry")
    };
    assert_eq!((x.len(), y.len()), (12, 12));
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn an_empty_graph_produces_empty_geometry() {
    let t = crate::index::empty_model();
    let geometry = BarnesHut::run(&t, &ForceParams::default()).expect("empty is finite");
    assert_eq!(
        geometry.nodes,
        NodeGeometry::Point {
            x: vec![],
            y: vec![]
        }
    );
}

#[test]
fn the_same_topology_settles_to_the_same_geometry_run_to_run() {
    let (nodes, edges) = line(30);
    let t = index_model(&nodes, &edges).expect("fits");
    let a = BarnesHut::run(&t, &ForceParams::default()).expect("finite");
    let b = BarnesHut::run(&t, &ForceParams::default()).expect("finite");
    assert_eq!(a, b, "pure: the same topology, the same run, every time");
}

/// A mid-degree node `mid` attached to `hubs` hubs, each hub also attached to
/// `leaves_per_hub` hubs-exclusive leaves — so every hub's degree dwarfs `mid`'s.
fn hub_fixture(hubs: u32, leaves_per_hub: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let mut nodes = vec![node("mid", "")];
    let mut edges = Vec::new();
    for h in 0..hubs {
        let hub_id = format!("hub{h}");
        nodes.push(node(&hub_id, ""));
        edges.push(edge(&format!("e-mid-{h}"), "mid", &hub_id));
        for l in 0..leaves_per_hub {
            let leaf_id = format!("leaf{h}_{l}");
            nodes.push(node(&leaf_id, ""));
            edges.push(edge(&format!("e-{h}-{l}"), &hub_id, &leaf_id));
        }
    }
    (nodes, edges)
}

/// Devil C9: run the full tick count and inspect raw simulation state (not just the
/// final `Stage::run` positions) so a divergence is caught even if it only shows up in
/// velocity, not position. If this fails, the fixture has found real Jacobi-link
/// instability: per C9, that is a stop-and-report, not a normalisation to improvise.
#[test]
fn the_jacobi_link_stability_fixture_stays_finite_and_bounded_devil_c9() {
    let (nodes, edges) = hub_fixture(6, 15);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&t, ForceParams::default(), 0);
    let mut energy = Vec::with_capacity(TICKS as usize);
    for _ in 0..TICKS {
        sim.tick();
        let e: f64 = sim
            .vx
            .iter()
            .zip(&sim.vy)
            .map(|(&vx, &vy)| vx * vx + vy * vy)
            .sum();
        assert!(
            e.is_finite(),
            "kinetic energy went non-finite mid-run: devil C9, stop and report"
        );
        energy.push(e);
    }
    let (x, y) = sim.positions();
    assert!(
        x.iter().chain(y).all(|v| v.is_finite()),
        "devil C9: position went non-finite"
    );
    let settled = &energy[energy.len() - 10..];
    let peak = settled.iter().cloned().fold(0.0_f64, f64::max);
    assert!(
        peak < 1.0,
        "devil C9: still oscillating in the last 10 ticks (peak KE {peak}), not settled"
    );
}
