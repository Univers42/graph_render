//! The three accessors' own cases: that the simple graph they hand back is the probe's own
//! and not a copy that could drift, that a fresh session is at rest, that a tick moves the
//! velocities, and that asking a question changes nothing about the next tick's bytes.
//!
//! Split from `columns.rs` by the house's 300-line limit, and because these need the gate's
//! own model (`mesh_probe/tests.rs`'s fixture) rather than a hand-made graph.

use crate::index::{Topology, index_model};
use crate::layout::force::{ForceSession, LiveParams};
use crate::stage::{gate_node_count, seeded_model};
use crate::weights::REFERENCE_DEGREE;

/// The gate's own model for `seed` (`prompt.md` §7.1), on the mesh: a connected graph of
/// 2 to 601 nodes with real degrees, which is the only kind a velocity means anything on.
fn topology(seed: u32) -> Topology {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the gate's model fits the u32 index space")
}

/// A mesh session over the gate's model for `seed`.
fn session(seed: u32) -> ForceSession {
    ForceSession::new(&topology(seed), LiveParams::default())
        .expect("the defaults are in range")
        .with_particle_mesh()
}

/// Every `f64` as its bits, so "equal" means bit-equal and not "close" — the same reason
/// `mesh_probe/tests.rs` and the wasm session's `fixture.rs` compare bits.
fn bits(column: &[f64]) -> Vec<u64> {
    column.iter().map(|v| v.to_bits()).collect()
}

/// The three columns are the probe's own, not a second copy: a device reading them and a
/// host reading the probe must see the same bytes, or the live input and the instrument
/// disagree about what the graph is.
#[test]
fn simple_edges_are_the_probes_own() {
    for seed in [3u32, 5, 9] {
        let session = session(seed);
        let probe = session.mesh_probe().expect("a field to solve");
        let (lo, hi, strength) = session.simple_edges();
        assert_eq!(lo, probe.lo.as_slice(), "seed {seed}: lo");
        assert_eq!(hi, probe.hi.as_slice(), "seed {seed}: hi");
        assert_eq!(strength, probe.strength.as_slice(), "seed {seed}: strength");
    }
}

/// A session that has not ticked is at rest: every velocity is exactly `0.0`, one per node.
#[test]
fn a_fresh_session_is_at_rest() {
    for seed in [3u32, 5, 9] {
        let session = session(seed);
        let n = session.xs().len();
        assert!(n > 1, "seed {seed}: the model has more than one node");
        assert_eq!(session.vxs().len(), n, "seed {seed}: one vx per node");
        assert_eq!(session.vys().len(), n, "seed {seed}: one vy per node");
        assert!(
            session.vxs().iter().all(|v| *v == 0.0),
            "seed {seed}: vx at rest"
        );
        assert!(
            session.vys().iter().all(|v| *v == 0.0),
            "seed {seed}: vy at rest"
        );
    }
}

/// One tick moves something, and the velocity columns stay one-per-node with the positions.
#[test]
fn the_velocities_move_with_a_tick() {
    for seed in [3u32, 5, 9] {
        let mut session = session(seed);
        session.step(1);
        assert!(
            session
                .vxs()
                .iter()
                .chain(session.vys().iter())
                .any(|v| *v != 0.0),
            "seed {seed}: a tick moved something"
        );
        assert_eq!(
            session.vxs().len(),
            session.xs().len(),
            "seed {seed}: one velocity per node"
        );
    }
}

/// Asking the session a question leaves the next tick's bytes alone: the accessors borrow,
/// they do not perturb. Two identical sessions, one interrogated and one not, must land on
/// the same `xs`, `ys`, `vxs` and `vys` after the same three ticks.
#[test]
fn the_accessors_leave_the_next_tick_byte_identical() {
    let topology = topology(5);
    let mut asked = ForceSession::new(&topology, LiveParams::default())
        .expect("the defaults are in range")
        .with_particle_mesh();
    let mut untouched = ForceSession::new(&topology, LiveParams::default())
        .expect("the defaults are in range")
        .with_particle_mesh();
    let (lo, hi, strength) = asked.simple_edges();
    let vxs = asked.vxs();
    let vys = asked.vys();
    assert_eq!(lo.len(), hi.len(), "one endpoint per edge");
    assert_eq!(lo.len(), strength.len(), "one strength per edge");
    assert!(vxs.len() > 1, "one velocity per node");
    assert_eq!(vxs.len(), vys.len(), "the two columns are the same length");
    asked.step(3);
    untouched.step(3);
    assert_eq!(bits(asked.xs()), bits(untouched.xs()), "x");
    assert_eq!(bits(asked.ys()), bits(untouched.ys()), "y");
    assert_eq!(bits(asked.vxs()), bits(untouched.vxs()), "vx");
    assert_eq!(bits(asked.vys()), bits(untouched.vys()), "vy");
}
