//! Carrying a running session onto a new topology: the growth path's positive and
//! negative controls.
//!
//! The whole design rests on one claim — **only ids cross a carry** — so half this file is
//! that claim both ways: survivors keep their bytes when the rows they land on move
//! ([`only_ids_cross_a_carry`]), and their positions are never the identity.

use super::support;
use crate::index::{Topology, index_model};
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow, SessionError};
use crate::records::{EdgeRecord, NodeRecord};
use crate::stage::seeded_model;
use crate::weights::REFERENCE_DEGREE;
use std::collections::HashSet;

/// The layout unit a new node with carried neighbours is offset by, recomputed here rather
/// than imported: a test that reads the implementation's own constant checks nothing about
/// the number it asserts.
const OFFSET_RADIUS: f64 = 1.0;

/// `barnes_hut/seed.rs`'s angle, for the same reason.
const GOLDEN_ANGLE: f64 = 2.399963229728653;

/// Carrying a session onto the topology it is already over changes nothing: every row is a
/// carried row, so the new session is the old one field for field and the two runs step to
/// the same bytes at one tick and at thirty.
#[test]
fn carrying_onto_the_same_topology_is_the_same_run() {
    let topology = support::topology(7);
    for k in [1, 30] {
        let mut straight = live(&topology);
        let mut carried = straight
            .carry(&topology, &topology)
            .expect("the topology is the one it was built over");
        straight.step(k);
        carried.step(k);
        assert_eq!(
            support::bits(&straight),
            support::bits(&carried),
            "{k} tick(s) of a carry onto the same topology"
        );
    }
}

/// The growth case: the model minus its last tenth, carried onto the whole model. Every
/// surviving node keeps its exact bytes, and the run's heat, tick count and parameters come
/// across too — a carry that restarted the schedule would look like a different picture from
/// the next tick on.
#[test]
fn a_grown_topology_keeps_every_old_node_byte_for_byte() {
    let models = models(400, 360);
    let old = live(&models.prefix);
    let carried = old
        .carry(&models.prefix, &models.full)
        .expect("`prefix` is what it was built over");
    let (ox, oy, ovx, ovy) = columns(&old);
    let (nx, ny, nvx, nvy) = columns(&carried);
    let kept = models.prefix.node_count() as usize;
    assert_eq!(&nx[..kept], &ox[..], "every surviving x, to the bit");
    assert_eq!(&ny[..kept], &oy[..], "every surviving y, to the bit");
    assert_eq!(&nvx[..kept], &ovx[..], "and every surviving vx");
    assert_eq!(&nvy[..kept], &ovy[..], "and every surviving vy");
    assert_eq!(carried.alpha(), old.alpha(), "the heat carries");
    assert_eq!(carried.params(), old.params(), "and the parameters");
    assert_eq!(
        carried.sim.tick_no, old.sim.tick_no,
        "and so does the tick count — a carry is not a restart"
    );
}

/// A new node is placed, not scattered: beside the mean of the neighbours it was actually
/// connected to, within the offset radius, at rest, and never on top of anything else.
#[test]
fn a_new_node_starts_beside_its_carried_neighbours_and_nowhere_else() {
    let models = models(400, 360);
    let old = live(&models.prefix);
    let carried = old
        .carry(&models.prefix, &models.full)
        .expect("`prefix` is what it was built over");
    let mut fresh = 0_u32;
    let mut placed = 0_u32;
    for row in models.prefix.node_count()..models.full.node_count() {
        let Some(mean) = carried_mean(&models, row, &old) else {
            fresh += 1;
            continue;
        };
        let at = row as usize;
        let (dx, dy) = (carried.xs()[at] - mean.0, carried.ys()[at] - mean.1);
        let chord = libm::sqrt(dx * dx + dy * dy);
        assert!(
            chord <= OFFSET_RADIUS + 1e-9,
            "row {row} is {chord} from its neighbours' mean, not within {OFFSET_RADIUS}"
        );
        let angle = f64::from(fresh) * GOLDEN_ANGLE;
        let want = (
            OFFSET_RADIUS * libm::cos(angle),
            OFFSET_RADIUS * libm::sin(angle),
        );
        let off = (libm::fabs(dx - want.0), libm::fabs(dy - want.1));
        assert!(
            off.0 <= 1e-9 && off.1 <= 1e-9,
            "the offset is the phyllotaxis one for its index among the new rows: \
             got ({dx}, {dy}), want {want:?} for index {fresh}"
        );
        assert_eq!(
            (carried.sim.vx[at], carried.sim.vy[at]),
            (0.0, 0.0),
            "a node that has never moved starts at rest"
        );
        fresh += 1;
        placed += 1;
    }
    assert!(
        placed > 0,
        "the model grew, so some new row has a carried neighbour"
    );
    assert!(
        distinct(carried.xs(), carried.ys()),
        "no two positions are equal"
    );
}

/// The shrink case: the model without its last tenth, carried onto what is left. Dropping a
/// node removes a row and nothing else — every survivor is still where it was.
#[test]
fn a_shrunk_topology_keeps_its_survivors_byte_for_byte() {
    let models = models(400, 360);
    let old = live(&models.full);
    let carried = old
        .carry(&models.full, &models.prefix)
        .expect("`full` is what it was built over");
    let (ox, oy, ovx, ovy) = columns(&old);
    let (nx, ny, nvx, nvy) = columns(&carried);
    let kept = models.prefix.node_count() as usize;
    assert_eq!((&nx[..kept], &ny[..kept]), (&ox[..kept], &oy[..kept]));
    assert_eq!((&nvx[..kept], &nvy[..kept]), (&ovx[..kept], &ovy[..kept]));
    assert!(
        nx[kept..].is_empty() && ny[kept..].is_empty() && nx.len() == kept,
        "a dropped node leaves no column behind"
    );
}

/// **The negative control for the design.** The same carry, onto the same ids, but with the
/// rows in the opposite order: every survivor must land on the row its *id* is at, at its own
/// bytes. A carry that indexed by position would keep row 0 where row 0 was and scramble the
/// picture, and this is the assertion that catches it.
#[test]
fn only_ids_cross_a_carry() {
    let models = models(400, 360);
    let old = live(&models.prefix);
    let mut carried = old
        .carry(&models.prefix, &models.shuffled)
        .expect("`prefix` is what it was built over");
    assert_ne!(
        carried.xs()[0],
        old.xs()[0],
        "the rows really are a different order, so the comparison below has teeth"
    );
    for row in 0..models.prefix.node_count() {
        let id = models.prefix.node(row).id.to_string();
        let here = models.shuffled.node_index(&id).expect("same ids") as usize;
        let there = row as usize;
        assert_eq!(
            (carried.xs()[here], carried.ys()[here]),
            (old.xs()[there], old.ys()[there]),
            "node {id} keeps its own position, on whatever row its id is at"
        );
    }
    let before = carried.xs().to_vec();
    carried.step(30);
    assert_ne!(before, carried.xs(), "and the carried session is live");
}

/// A pin is a node's state as much as its position is: it carries to the same node in the
/// new topology, and that node is still *placed* by the next tick rather than integrated.
#[test]
fn a_pin_carries_with_its_node() {
    let models = models(400, 360);
    let mut old = live(&models.prefix);
    old.pin(NodeRow::new(3), 100.0, -50.0)
        .expect("row 3 exists");
    old.step(10);
    let id = models.prefix.node(3).id.to_string();
    let mut carried = old
        .carry(&models.prefix, &models.full)
        .expect("`prefix` is what it was built over");
    let row = models.full.node_index(&id).expect("the grown model has it") as usize;
    carried.step(1);
    assert_eq!(
        (carried.xs()[row], carried.ys()[row]),
        (100.0, -50.0),
        "the pin held the node at the same coordinates across the carry"
    );
    carried.step(20);
    assert_eq!(
        (carried.xs()[row], carried.ys()[row]),
        (100.0, -50.0),
        "and it is still a pin twenty ticks later, not a starting point"
    );
}

/// A `from` that is not the topology the session is over cannot be mapped id for id at all,
/// and nothing is built when it is refused.
#[test]
fn a_carry_from_the_wrong_number_of_rows_is_refused() {
    let models = models(400, 360);
    let session = live(&models.prefix);
    assert_eq!(
        session.carry(&models.full, &models.full).err(),
        Some(SessionError::ColumnLength {
            column: "from",
            got: u64::from(models.full.node_count()),
            nodes: models.prefix.node_count(),
        })
    );
}

/// The three topologies every case here carries between: the gate's own 400-node model,
/// the same model without its last 40 nodes and the edges among those kept, and the whole
/// model with its nodes in the opposite order.
///
/// The kept prefix keeps its rows — `index_model` assigns them in first-seen order — so the
/// byte comparisons above are between like rows, and `shuffled` is the one place the rows
/// are deliberately moved.
struct Models {
    prefix: Topology,
    full: Topology,
    shuffled: Topology,
}

fn models(count: u32, keep: u32) -> Models {
    let (nodes, edges) = seeded_model(7, count, REFERENCE_DEGREE);
    let kept: HashSet<&str> = nodes[..keep as usize]
        .iter()
        .map(|node| node.id.as_str())
        .collect();
    let prefix_nodes: Vec<NodeRecord> = nodes[..keep as usize].to_vec();
    let prefix_edges: Vec<EdgeRecord> = edges
        .iter()
        .filter(|edge| kept.contains(edge.source.as_str()) && kept.contains(edge.target.as_str()))
        .cloned()
        .collect();
    let mut reversed = nodes.clone();
    reversed.reverse();
    Models {
        prefix: index(&prefix_nodes, &prefix_edges),
        full: index(&nodes, &edges),
        shuffled: index(&reversed, &edges),
    }
}

/// A session hot enough that its velocities and positions are worth comparing: three ticks
/// to get off the seed, a reheat, then ten more so nothing has settled back to a fixed point.
fn live(topology: &Topology) -> ForceSession {
    let mut session =
        ForceSession::new(topology, LiveParams::default()).expect("the defaults are in range");
    session.step(3);
    session.reheat(1.0).expect("1.0 is in range");
    session.step(10);
    session
}

fn index(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Topology {
    index_model(nodes, edges).expect("the gate's model fits the u32 index space")
}

/// `session`'s four columns as bit patterns: `x`, `y`, `vx`, `vy`. Read from its own `Sim`
/// because there is no public velocity column (`xs`/`ys` are positions), and the claim under
/// test is about the columns, not about what a later tick makes of them.
fn columns(session: &ForceSession) -> (Vec<u64>, Vec<u64>, Vec<u64>, Vec<u64>) {
    let sim = &session.sim;
    (bits(&sim.x), bits(&sim.y), bits(&sim.vx), bits(&sim.vy))
}

fn bits(column: &[f64]) -> Vec<u64> {
    column.iter().map(|v| v.to_bits()).collect()
}

/// The mean of `row`'s neighbours in the full model that are rows of the prefix, or `None`
/// when it has none. Walked from the topology's own edges rather than reused from `carry`, so
/// the assertion is about the rule and not about its implementation.
fn carried_mean(models: &Models, row: u32, old: &ForceSession) -> Option<(f64, f64)> {
    let edges = models.full.edges();
    let (mut sum_x, mut sum_y, mut count) = (0.0_f64, 0.0_f64, 0_u32);
    for e in models.full.incident(row) {
        let other = if edges.source[e as usize] == row {
            edges.target[e as usize]
        } else {
            edges.source[e as usize]
        };
        let Some(row) = models.prefix.node_index(models.full.node(other).id) else {
            continue;
        };
        sum_x += old.xs()[row as usize];
        sum_y += old.ys()[row as usize];
        count += 1;
    }
    (count > 0).then(|| (sum_x / f64::from(count), sum_y / f64::from(count)))
}

/// Every `(x, y)` distinct, by bit pattern: a coincidence the layout separates on its next
/// tick, and one a carry must not create.
fn distinct(xs: &[f64], ys: &[f64]) -> bool {
    let pairs: HashSet<(u64, u64)> = xs
        .iter()
        .zip(ys)
        .map(|(&x, &y)| (x.to_bits(), y.to_bits()))
        .collect();
    pairs.len() == xs.len()
}
