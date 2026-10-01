//! The collide and link range kernels, as tests: the same deltas at every worker count,
//! the same numbers as the loops they replace, the tick handing exactly the listed
//! passes to the runner, and each pass's own split-sum control moving the layout.

use super::super::charge;
use super::super::collide;
use super::super::link;
use super::super::sim::{How, Sim};
use super::super::step::{CollidePass, LinkPass, Pass};
use super::super::{BarnesHut, Split};
use super::line;
use crate::exec::{Runner, Serial, StepRange};
use crate::index::index_model;
use crate::layout::force::params::ForceParams;
use std::cell::Cell as Counter;

/// A runner that counts the passes handed to it, then runs them on the calling thread, so
/// a test can ask the stage *how many* passes it divides.
///
/// It cannot ask *which*: the three kernels share an output type and a length, so nothing
/// outside [`Sim::tick`] can tell them apart. That is why the count is what is checked and
/// the order in [`BarnesHut::THREADED_PASSES`] is a stated fact about the tick's body
/// rather than a tested one.
struct Counting<'a>(&'a Counter<u32>);

impl Runner for Counting<'_> {
    fn run<O: StepRange>(&self, kernel: &O, workers: u32, out: &mut Vec<O::Out>) {
        self.0.set(self.0.get() + 1);
        Serial.run(kernel, workers, out);
    }
}

#[test]
fn collide_gives_the_same_deltas_at_every_worker_count_as_the_loop_it_replaces() {
    let (nodes, edges) = line(40);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&t, ForceParams::default().into(), 0);
    collide::prepare(&mut sim);
    let mut reference = Vec::new();
    Serial.run(&CollidePass::of(&sim), 1, &mut reference);
    assert_eq!(reference.len(), 40);
    assert!(
        reference.iter().any(|&(x, y)| x != 0.0 || y != 0.0),
        "the pass must do real work, or the equality below is vacuous"
    );
    for workers in [0_u32, 1, 2, 3, 4, 7, 8, 64] {
        let mut out = Vec::new();
        Serial.run(&CollidePass::of(&sim), workers, &mut out);
        assert_eq!(out, reference, "workers={workers} moved a collide delta");
    }
    // The kernel is the loop it replaces: one walk per node, in the collide tree's order.
    let reach = collide::reach_squared(&sim);
    let through_loop: Vec<(f64, f64)> = (sim.collide_tree.order().iter())
        .map(|&i| collide::node_delta(&sim, i, reach))
        .collect();
    assert_eq!(reference, through_loop);
}

#[test]
fn link_gives_the_same_deltas_at_every_worker_count_as_the_loop_it_replaces() {
    let (nodes, edges) = line(40);
    let t = index_model(&nodes, &edges).expect("fits");
    let sim = Sim::new(&t, ForceParams::default().into(), 0);
    let mut reference = Vec::new();
    Serial.run(&LinkPass::of(&sim), 1, &mut reference);
    assert_eq!(reference.len(), 40);
    assert!(
        reference.iter().any(|&(x, y)| x != 0.0 || y != 0.0),
        "the pass must do real work, or the equality below is vacuous"
    );
    for workers in [0_u32, 1, 2, 3, 4, 7, 8, 64] {
        let mut out = Vec::new();
        Serial.run(&LinkPass::of(&sim), workers, &mut out);
        assert_eq!(out, reference, "workers={workers} moved a link delta");
    }
    // The kernel is the loop it replaces: every simple edge once, in ascending edge order,
    // scattered into both its endpoints (`link::scatter`).
    let mut through_loop = vec![(0.0, 0.0); sim.x.len()];
    for e in 0..sim.graph.lo.len() {
        link::scatter(&sim, e, &mut through_loop);
    }
    assert_eq!(reference, through_loop);
}

/// The tick hands the runner one call per pass in [`BarnesHut::THREADED_PASSES`].
///
/// The list is what a measurement report prints as a speedup's denominator and what
/// `hashgate`'s `GM_MUTATE_SPLIT_SUM` names, so a pass threaded without being listed would
/// make every number downstream wrong while every test that only checked hashes still
/// passed. This is the count that catches it.
#[test]
fn the_tick_hands_the_runner_one_call_per_listed_pass() {
    let (nodes, edges) = line(12);
    let t = index_model(&nodes, &edges).expect("fits");
    let calls = Counter::new(0);
    let mut sim = Sim::new(&t, ForceParams::default().into(), 0);
    let mut deltas = Vec::new();
    let mut how = How {
        runner: &Counting(&calls),
        workers: 3,
        deltas: &mut deltas,
        split: Split::None,
    };
    sim.tick(&mut how);
    assert_eq!(
        calls.get(),
        BarnesHut::THREADED_PASSES.len() as u32,
        "a pass threaded without being listed would misattribute every speedup"
    );
    assert_eq!(
        BarnesHut::THREADED_PASSES,
        ["link", "charge", "collide"],
        "the list is in Sim::tick's own order, with center never threaded"
    );
}

#[test]
fn each_passes_own_split_control_moves_the_layout_and_none_of_them_does_when_off() {
    let (nodes, edges) = line(40);
    let t = index_model(&nodes, &edges).expect("fits");
    let params = ForceParams::default();
    let honest = BarnesHut::run_with(&t, &params, &Serial, 1).expect("finite");
    for split in [Split::Charge, Split::Collide, Split::Link] {
        let mutated = BarnesHut::run_under(&t, &params, &Serial, 1, split).expect("finite");
        assert_ne!(
            mutated, honest,
            "{split:?} changed nothing, so the control would pass a gate"
        );
        // The control is the same whatever the worker count, so a threaded arm that
        // ignored it would be distinguishable from one that applied it.
        for workers in [4_u32, 7] {
            let threaded =
                BarnesHut::run_under(&t, &params, &Serial, workers, split).expect("finite");
            assert_eq!(
                threaded,
                BarnesHut::run_under(&t, &params, &Serial, workers + 1, split).expect("finite"),
                "{split:?} at {workers} workers is not worker-count invariant"
            );
        }
    }
    assert_eq!(
        BarnesHut::run_under(&t, &params, &Serial, 1, Split::None).expect("finite"),
        honest,
        "no control must be the stage itself"
    );
}

#[test]
fn the_three_passes_share_one_sim_and_one_scratch_buffer() {
    // A tick runs all three over the same columns and the same `deltas` buffer, so each
    // pass must leave the buffer and the state the next one reads in the shape it expects.
    let (nodes, edges) = line(24);
    let t = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&t, ForceParams::default().into(), 0);
    let mut deltas = Vec::new();
    charge::prepare(&mut sim);
    Serial.run(&Pass::of(&sim), 1, &mut deltas);
    let after_charge = deltas.len();
    collide::prepare(&mut sim);
    Serial.run(&CollidePass::of(&sim), 1, &mut deltas);
    let after_collide = deltas.len();
    Serial.run(&LinkPass::of(&sim), 1, &mut deltas);
    assert_eq!((after_charge, after_collide, deltas.len()), (24, 24, 24));
    assert_eq!(Pass::of(&sim).len(), 24);
    assert_eq!(CollidePass::of(&sim).len(), 24);
    assert_eq!(LinkPass::of(&sim).len(), 24);
}

/// **A control that passes vacuously is worse than no control**, and each pass's own
/// control bites at a different seed count — this is the measurement of that.
///
/// The gate's model is `gate_node_count(seed) = 2 + seed % 600` nodes. At two or three
/// nodes, link and many-body push the pair past `2 * collideRadius` before collide runs, so
/// collide has no overlap left to resolve, its deltas are zero, and **no split of its merge
/// can move anything**: the first seed whose collide pass does real work is seed 4 (six
/// nodes). Charge and link bite at seed 0. So a collide control row has to reach at least
/// five seeds to be a control at all, and this test is what says so — the same trap
/// `cli_force.rs` records for the theta knob.
#[test]
fn every_passs_own_control_bites_by_the_gate_s_fifth_seed() {
    // The seed each control first bites at, which is the number a gate row must reach.
    let mut first: Vec<(Split, u32)> = Vec::new();
    for seed in 0..5u32 {
        let count = crate::gate_node_count(seed);
        let (nodes, edges) = crate::seeded_model(seed, count, crate::REFERENCE_DEGREE);
        let t = index_model(&nodes, &edges).expect("fits");
        let params = ForceParams::default();
        let honest = BarnesHut::run_with(&t, &params, &Serial, 1).expect("finite");
        for split in [Split::Charge, Split::Collide, Split::Link] {
            let moved =
                BarnesHut::run_under(&t, &params, &Serial, 1, split).expect("finite") != honest;
            if moved && !first.iter().any(|(seen, _)| *seen == split) {
                first.push((split, seed));
            }
        }
    }
    assert_eq!(
        first,
        vec![(Split::Charge, 0), (Split::Link, 0), (Split::Collide, 4)],
        "a control that never bites would pass every gate row it is filed under"
    );
}
