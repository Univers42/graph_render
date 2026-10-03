//! The divided aggregate against the serial one: same cells, same `order`, same bodies, at
//! every worker count. Split into its own file for the house line cap, the way `charge.rs`
//! already splits `tests.rs` and `step.rs`.

use super::super::aggregate::{self, Pass, finish};
use super::super::{Body, prepare, prepare_with};
use crate::REFERENCE_DEGREE;
use crate::exec::{Runner, Serial, StepRange, range_at};
use crate::index::index_model;
use crate::layout::force::barnes_hut::sim::Sim;
use crate::layout::force::params::ForceParams;
use crate::layout::force::quadtree::{Cell, Quadtree};
use crate::stage::{gate_node_count, seeded_model};

/// The gate's worker counts, plus the eight a 1M tick is measured at: the prologue's claim
/// is a worker count is a schedule, so the counts are the ones the gates and the bench use.
const WORKERS: [u32; 8] = [1, 2, 3, 4, 5, 7, 8, 16];

/// One arena plus its aggregate, as the plain numbers a comparison can hold: `f64`s as
/// their bit patterns, so "equal" is the byte claim and not a tolerance.
#[derive(Clone, Debug, PartialEq)]
struct Arena {
    cells: Vec<(u64, u64, u64, u32, u32, u32)>,
    order: Vec<u32>,
    node_id: Vec<u32>,
    bodies: Vec<(u64, u64, u64, u32, u32, u32)>,
}

impl Arena {
    fn of(sim: &Sim) -> Self {
        let tree = &sim.charge_tree;
        let cells = tree.cells().iter().map(cell_bits).collect();
        let node_id = (0..tree.cells().len() as u32)
            .map(|k| tree.node_id(k))
            .collect();
        let bodies = sim
            .bodies
            .iter()
            .map(|b| {
                (
                    b.comx.to_bits(),
                    b.comy.to_bits(),
                    b.open.to_bits(),
                    b.count,
                    b.skip,
                    b.start,
                )
            })
            .collect();
        Self {
            cells,
            order: tree.order().to_vec(),
            node_id,
            bodies,
        }
    }
}

fn cell_bits(cell: &Cell) -> (u64, u64, u64, u32, u32, u32) {
    let b = cell.bounds;
    (
        b.x0.to_bits(),
        b.y0.to_bits(),
        b.x1.to_bits(),
        cell.skip,
        cell.start,
        cell.end,
    )
}

/// A session over the gate's model for `seed`, aggregated over `workers` ranges.
fn prepared(seed: u32, workers: u32) -> Sim {
    seeded_sim(seed, gate_node_count(seed), workers)
}

fn seeded_sim(seed: u32, count: u32, workers: u32) -> Sim {
    let (nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("the gate model indexes");
    let mut sim = Sim::new(&topology, ForceParams::default().into(), seed);
    prepare_with(&mut sim, &Serial, workers);
    sim
}

/// **The claim.** A Barnes-Hut tick's prologue is one program at every worker count: the
/// arena, its point order, its shape-slot ids (the charge walk's jiggle keys) and every
/// cell's body are the same bytes whether the aggregate ran as one range or sixteen.
///
/// The gate's own seeds are 2 to 8 nodes, so the second loop is a 400-node model: a cut
/// only happens where a range boundary lands inside a subtree, which needs more cells than
/// workers, and eight nodes never has that.
#[test]
fn the_arena_and_the_bodies_are_worker_count_invariant_over_the_gate_seeds() {
    for seed in 0..8 {
        assert_invariant(seed, gate_node_count(seed), "gate seed");
    }
    for seed in 0..4 {
        assert_invariant(seed, 400, "400-node model");
    }
}

fn assert_invariant(seed: u32, count: u32, what: &str) {
    let serial = Arena::of(&seeded_sim(seed, count, 1));
    if count > 100 {
        assert!(
            serial.cells.len() > WORKERS.len(),
            "{what} {seed}: {} cells, too few to cut",
            serial.cells.len()
        );
    }
    for workers in WORKERS {
        assert_eq!(
            Arena::of(&seeded_sim(seed, count, workers)),
            serial,
            "{what} {seed} at {workers} workers"
        );
    }
}

/// Coincident points are the case the aggregate has to get right and the split has to leave
/// alone: a chain of points at one position shares one leaf, so several cells read the same
/// chain head, and `insert_leaf`'s bail chains points the tree cannot separate. Hand-built
/// rather than laid out, because `Sim::new`'s seed positions are distinct by construction.
#[test]
fn a_parallel_aggregate_over_coincident_points_is_the_serial_one() {
    let xs = [1.0, 1.0, 1.0, 2.0, -3.5, 40.25, 40.25, 0.0];
    let ys = [2.0, 2.0, 2.0, 2.0, 7.0, -1.0, -1.0, 0.0];
    let mut tree = Quadtree::default();
    tree.build(&xs, &ys);
    let pass = Pass::of(&tree, (&xs, &ys), 0.9);
    let mut serial = vec![Body::default(); pass.cells.len()];
    aggregate::serial(&pass, &mut serial);
    for workers in WORKERS {
        let mut divided = vec![Body::default(); serial.len()];
        Serial.run(&pass, workers, &mut divided);
        finish(&pass, &mut divided);
        assert_eq!(bits(&divided), bits(&serial), "{workers} workers");
    }
}

/// The division is the runner's, not `workers`': the wasm pool cuts a pass into 16 chunks
/// per thread. Chunk counts that are no worker count, over a model deep enough that every
/// boundary cuts a subtree, must still finish to the serial bodies.
#[test]
fn any_division_of_the_aggregate_finishes_to_the_serial_one() {
    let sim = seeded_sim(5, 400, 1);
    let pass = Pass::of(&sim.charge_tree, (&sim.x, &sim.y), sim.params.theta);
    let mut serial = vec![Body::default(); pass.cells.len()];
    aggregate::serial(&pass, &mut serial);
    for chunks in [2, 3, 16, 48, 112, pass.len()] {
        let mut divided = vec![Body::default(); serial.len()];
        for i in 0..chunks {
            let range = range_at(pass.len(), chunks, i);
            let span = &mut divided[range.start as usize..range.end as usize];
            pass.step_range(range, span);
        }
        finish(&pass, &mut divided);
        assert_eq!(bits(&divided), bits(&serial), "{chunks} chunks");
    }
}

fn bits(bodies: &[Body]) -> Vec<(u64, u64, u64, u32, u32, u32)> {
    bodies
        .iter()
        .map(|b| {
            (
                b.comx.to_bits(),
                b.comy.to_bits(),
                b.open.to_bits(),
                b.count,
                b.skip,
                b.start,
            )
        })
        .collect()
}

/// The negative control for the comparison above. Two **sibling** cells — two children of
/// the same parent — with their bodies exchanged: the arena still has the right cells, the
/// right order and the right number of bodies, and every body is a real one from the same
/// tick, so a comparison that only looked at shape or at a digest per cell would call this
/// equal. It must not be.
#[test]
fn two_sibling_cells_with_their_bodies_swapped_are_caught() {
    let sim = seeded_sim(0, 400, 4);
    let honest = Arena::of(&sim);
    let (a, b) = siblings(&sim);
    assert_ne!(
        honest.bodies[a], honest.bodies[b],
        "the two siblings must differ to be told apart"
    );
    let mut swapped = honest.clone();
    (swapped.bodies[a], swapped.bodies[b]) = (swapped.bodies[b], swapped.bodies[a]);
    assert_ne!(
        swapped, honest,
        "a swapped sibling compared equal: the test proves nothing"
    );
}

/// Two children of the same parent, both leaves: cells `k + 1` and `k + 2` are read by one
/// `centre` call for their parent, and each is read by every query that descends there.
fn siblings(sim: &Sim) -> (usize, usize) {
    let cells = sim.charge_tree.cells();
    (1..cells.len() - 2)
        .map(|k| k as u32)
        .find(|&k| {
            cells[k as usize].skip > k + 1
                && cells[k as usize + 1].skip == k + 2
                && cells[k as usize + 2].skip <= cells[k as usize].skip
        })
        .map(|k| (k as usize + 1, k as usize + 2))
        .expect("a cell with two leaf children")
}

/// The serial prologue is still the serial prologue: one worker over `Serial` is the loop
/// the tests compare against, not a lookalike that happens to agree.
#[test]
fn one_worker_over_serial_is_the_serial_loop() {
    let (nodes, edges) = seeded_model(3, gate_node_count(3), REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("the gate model indexes");
    let mut sim = Sim::new(&topology, ForceParams::default().into(), 3);
    prepare(&mut sim);
    assert_eq!(Arena::of(&sim), Arena::of(&prepared(3, 1)));
}
