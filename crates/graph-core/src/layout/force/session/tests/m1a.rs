//! **M1a — the frozen layout, byte for byte.** A default session stepped `TICKS` times is
//! what `layout.force.barnes_hut` now *is*, so its 4-way-hashed bytes must not move.
//!
//! The digests were taken from the stage **before** the refactor, over the gate's own
//! model for seeds 0..64 (`golden.rs`). They are asserted on the session's own output *and*
//! on the stage's, so neither can drift past the other: a change that moved both together
//! — the only way a refactor of this shape could quietly pass — fails here.

use super::golden::FROZEN;
use super::support;
use crate::index::Topology;
use crate::layout::force::BarnesHut;
use crate::layout::force::params::{ForceParams, TICKS};
use crate::layout::force::session::{ForceSession, LiveParams};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

#[test]
fn the_default_session_reproduces_the_frozen_layout_on_every_seed() {
    for seed in 0..FROZEN.len() as u32 {
        let topology = support::topology(seed);
        let mut session =
            ForceSession::new(&topology, LiveParams::default()).expect("the defaults are in range");
        session.step(TICKS);
        let (xs, ys) = support::as_f32(&session);
        assert_eq!(
            support::digest32(&xs, &ys),
            FROZEN[seed as usize],
            "seed {seed}: the session's own columns"
        );
        assert_eq!(
            (xs, ys),
            stage_columns(&topology),
            "seed {seed}: the stage runs the very session this test steps by hand"
        );
    }
}

/// The stage's own point columns — the bytes the snapshot and so the hash gate are taken
/// over.
fn stage_columns(topology: &Topology) -> (Vec<f32>, Vec<f32>) {
    let geometry = BarnesHut::run(topology, &ForceParams::default()).expect("finite");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("force layout is point geometry")
    };
    (x, y)
}

/// The frozen stage is the *degenerate* session, and nothing else: no pins, the default
/// parameters, `alpha_target` 0. The statement is the refactor's premise, so it is pinned
/// rather than left in a comment.
#[test]
fn the_frozen_layout_is_a_default_session_that_settles() {
    let mut session = support::session(3);
    assert_eq!(
        session.params(),
        LiveParams::default(),
        "the default parameters are the frozen force set, field for field"
    );
    let report = session.step(TICKS);
    assert!(
        report.settled,
        "alpha {} below alpha_min, alpha_target 0",
        report.alpha
    );
    assert!(
        session
            .xs()
            .iter()
            .chain(session.ys())
            .all(|v| v.is_finite())
    );
}
