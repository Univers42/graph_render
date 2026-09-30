//! Split from `fdeb.rs` for the house line cap. Includes the devil C9 fixture: a
//! mid-degree node wired to several much higher-degree hubs, the shape the Jacobi
//! reformulation of link (devil C7) is most at risk from, since every hub pulls from a
//! fixed pre-tick snapshot instead of seeing the others' corrections mid-pass.

#[cfg(test)]
mod kernels;

use super::charge;
use super::sim::{How, Sim};
use super::step::Pass;
use super::{BarnesHut, Split, TICKS};
use crate::exec::{Runner, Serial};
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
/// velocity, not in position. If this fails, the fixture has found real Jacobi-link
/// instability: per C9, that is a stop-and-report, not a normalisation to improvise.
#[test]
fn the_jacobi_link_stability_fixture_stays_finite_and_bounded_devil_c9() {
    let (nodes, edges) = hub_fixture(6, 15);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&t, ForceParams::default().into(), 0);
    let mut deltas = Vec::new();
    let mut energy = Vec::with_capacity(TICKS as usize);
    for _ in 0..TICKS {
        let mut how = How {
            runner: &Serial,
            workers: 1,
            deltas: &mut deltas,
            split: Split::None,
        };
        sim.tick(&mut how);
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
    let (x, y) = (&sim.x, &sim.y);
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

/// A settled enough state that the pass has real work to do: a few ticks in, so the
/// quadtree is not over a spiral and the aggregate is not all unit masses.
fn prepared(n: u32, ticks: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>, Sim) {
    let (nodes, edges) = line(n);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&t, ForceParams::default().into(), 0);
    for _ in 0..ticks {
        sim.alpha += -sim.alpha * sim.params.alpha_decay;
        charge::prepare(&mut sim);
        let mut deltas = Vec::new();
        Serial.run(&Pass::of(&sim), 1, &mut deltas);
        for (i, (dvx, dvy)) in deltas.into_iter().enumerate() {
            sim.vx[i] += dvx;
            sim.vy[i] += dvy;
        }
        sim.integrate();
        sim.tick_no += 1;
    }
    (nodes, edges, sim)
}

/// The range kernel's own claim, as a test: the same deltas at every worker count, the odd
/// ones included. This is the *partition* test — [`Serial`] running one range at a time, so
/// a range that read another range's output, or skipped a boundary, or walked a different
/// subtree, would show. The threaded runner over the same kernel is graph-cli's test,
/// where a thread can actually be spawned.
#[test]
fn the_pass_gives_the_same_deltas_at_every_worker_count() {
    let (_, _, sim) = prepared(40, 3);
    let mut reference = Vec::new();
    Serial.run(&Pass::of(&sim), 1, &mut reference);
    assert_eq!(reference.len(), 40);
    assert!(
        reference.iter().any(|&(x, y)| x != 0.0 || y != 0.0),
        "the pass must do real work, or the equality below is vacuous"
    );
    for workers in [0_u32, 1, 2, 3, 4, 7, 8, 64] {
        let mut out = Vec::new();
        Serial.run(&Pass::of(&sim), workers, &mut out);
        assert_eq!(out, reference, "workers={workers} moved a delta");
    }
}

/// The kernel and the serial loop it replaces are the same computation. Without this the
/// equality test above would pass on a kernel that computes something else entirely, one
/// worker or seven: it is the test that says `Pass` is `charge::apply`'s inner `for` loop
/// with a range, not a parallel reimplementation of the pass.
#[test]
fn the_kernel_and_the_serial_loop_are_the_same_computation() {
    let (nodes, edges) = line(40);
    let t = index_model(&nodes, &edges).expect("fits");
    let params = ForceParams::default();
    let mut sim = Sim::new(&t, params.into(), 0);
    charge::prepare(&mut sim);
    let mut through_kernel = Vec::new();
    Serial.run(&Pass::of(&sim), 1, &mut through_kernel);
    let mut through_loop = vec![(0.0, 0.0); sim.x.len()];
    let mut stack = Vec::new();
    for (i, slot) in through_loop.iter_mut().enumerate() {
        *slot = sim.node_delta(i as u32, &mut stack);
    }
    assert_eq!(through_kernel, through_loop);
}

/// The negative control for the pass's claim: a kernel that *sums* a node's delta into
/// its neighbour's — the scatter D10 forbids, and the shape a wrong partition would take —
/// must not compare equal. Without this, "every worker count agrees" could mean the range
/// boundaries are simply never exercised.
#[test]
fn a_pass_that_scatters_into_another_nodes_delta_is_caught() {
    let (_, _, sim) = prepared(24, 2);
    let mut honest = Vec::new();
    Serial.run(&Pass::of(&sim), 3, &mut honest);
    // The same numbers, each also added into the next node's slot: every element still
    // written exactly once by exactly one range, so the plan still looks right.
    let mut scattered = honest.clone();
    for i in 0..scattered.len() - 1 {
        scattered[i + 1].0 += scattered[i].1;
        scattered[i + 1].1 += scattered[i].0;
    }
    assert_ne!(
        scattered, honest,
        "a scatter compared equal: the equality tests prove nothing"
    );
}

/// The tree is built once per pass, ahead of the ranges, and every worker reads the same
/// one. A second build inside the kernel would be both wrong (a different arena order) and
/// the thing a thread could not do safely; this pins that `prepare` is the only builder.
#[test]
fn the_tree_is_built_once_before_the_ranges_and_read_only_inside_them() {
    let (_, _, sim) = prepared(16, 1);
    let mut first = Vec::new();
    Serial.run(&Pass::of(&sim), 1, &mut first);
    // Running the same prepared pass again gives the same numbers: nothing in the kernel
    // mutates the tree or the aggregate it reads.
    let mut second = Vec::new();
    Serial.run(&Pass::of(&sim), 7, &mut second);
    assert_eq!(first, second);
}

/// **The stage's claim, end to end.** The whole layout — every tick, every pass — is the
/// same bytes at every worker count, and `run_with(&Serial, 1)` is `Stage::run` exactly.
///
/// This is the unit-level twin of the N-way hash gate's threaded arm: the gate proves it
/// over 1000 seeds of the real registry, and this proves it on a fixture small enough to
/// read. Both matter, because the gate could pass with a wrong `run_with` if every
/// threaded arm were wrong *the same way* and only the serial arm were pinned elsewhere.
#[test]
fn the_whole_layout_is_worker_count_invariant_and_serial_one_is_the_stage() {
    let (nodes, edges) = line(50);
    let t = index_model(&nodes, &edges).expect("fits");
    let params = ForceParams::default();
    let reference = BarnesHut::run_with(&t, &params, &Serial, 1).expect("finite");
    assert_eq!(
        reference,
        BarnesHut::run(&t, &params).expect("finite"),
        "one worker over Serial must be the stage itself, not a lookalike"
    );
    for workers in [0_u32, 2, 3, 4, 7, 8, 16] {
        let out = BarnesHut::run_with(&t, &params, &Serial, workers).expect("finite");
        assert_eq!(out, reference, "workers={workers} moved a position");
    }
}

/// The negative control for that claim, at the same scope: `run_under`'s `split_sum`
/// control must produce a *different* layout, or `run_with` would be free to ignore the
/// merge entirely and the invariance test above would pass for a reason unrelated to the
/// partition.
///
/// This is the unit twin of the `GM_MUTATE_SPLIT_SUM` gate row: the control is a
/// **compiled-in parameter**, so a test can call it directly and a gate row can reach it
/// through the same argument, and neither depends on the other to exist.
#[test]
fn the_split_sum_control_moves_the_layout() {
    let (nodes, edges) = line(40);
    let t = index_model(&nodes, &edges).expect("fits");
    let params = ForceParams::default();
    let honest = BarnesHut::run_with(&t, &params, &Serial, 1).expect("finite");
    for workers in [1_u32, 2, 3, 4, 7] {
        let mutated =
            BarnesHut::run_under(&t, &params, &Serial, workers, Split::Charge).expect("finite");
        assert_ne!(
            mutated, honest,
            "workers={workers}: the split-sum control changed nothing, so it would pass a gate"
        );
    }
    // And it is the *same* control whatever the worker count, so a threaded arm that
    // ignored it would be distinguishable from one that applied it.
    let threaded = BarnesHut::run_under(&t, &params, &Serial, 4, Split::Charge).expect("finite");
    assert_eq!(
        threaded,
        BarnesHut::run_under(&t, &params, &Serial, 7, Split::Charge).expect("finite")
    );
}
