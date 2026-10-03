//! What the five cases share: the gate's own synthetic topology, a default session over
//! it, the byte-level views the assertions compare, and the `M1c` command script.

use super::digest::sha256_hex;
use crate::index::{Topology, index_model};
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};
use crate::stage::{gate_node_count, seeded_model};
use crate::weights::REFERENCE_DEGREE;

/// The model every verb case runs: 7 nodes, so every force has a neighbour to push against
/// and a pair far enough apart to push.
pub(super) const SEED: u32 = 5;

/// How many ticks both runs are stepped *before* the verb, so the forces have real
/// velocities rather than the zero row the spiral seed starts from — a verb applied to
/// an unmoved layout proves less than one applied to a running one.
pub(super) const WARMUP: u32 = 12;

/// The row `pin` and the release cases address, and a pin far outside anything the layout
/// reaches on its own, so a pinned row cannot land where it would have landed anyway.
pub(super) const ROW: u32 = 0;
pub(super) const PX: f64 = -250.0;
pub(super) const PY: f64 = 175.0;

/// Two runs of the same model, stepped in lockstep, one of which will be told something
/// the other is not. The whole of `verbs.rs` and `pins.rs` is the distance between them.
pub(super) struct Twin {
    /// The run nothing is ever said to: what the subject is measured against.
    pub(super) reference: ForceSession,
    /// The run every verb goes to.
    pub(super) subject: ForceSession,
}

impl Twin {
    /// Two fresh sessions over the gate's model for `seed`. A pair that differed by one
    /// field would make every assertion compare two different problems.
    pub(super) fn twinned(seed: u32) -> Self {
        let topology = topology(seed);
        let new = || ForceSession::new(&topology, LiveParams::default()).expect("in range");
        Self {
            reference: new(),
            subject: new(),
        }
    }

    /// Steps both runs the same number of ticks — the only thing that may differ between
    /// two runs of one model, and what `m1b` pins.
    pub(super) fn step(&mut self, ticks: u32) {
        self.reference.step(ticks);
        self.subject.step(ticks);
    }

    /// The two runs are the same bytes, `when` naming the point in the contract.
    pub(super) fn assert_same(&self, when: &str) {
        assert_eq!(bits(&self.subject), bits(&self.reference), "{when}");
    }

    /// The two runs are not the same bytes, `when` naming what was supposed to move.
    pub(super) fn assert_differ(&self, when: &str) {
        assert_ne!(bits(&self.subject), bits(&self.reference), "{when}");
    }
}

/// Steps the first two steps of the contract for a changed parameter: both runs warm, both
/// identical, the verb on the subject alone, and the positions **still** identical because
/// a verb only ever arms the next tick. Returns the pair one tick later, for the caller to
/// say what its force did.
pub(super) fn diverge(seed: u32, force: &str, params: LiveParams) -> Twin {
    let mut twin = Twin::twinned(seed);
    twin.step(WARMUP);
    twin.assert_same("two runs of one model are the same bytes before any verb");
    twin.subject
        .set_params(params)
        .unwrap_or_else(|_| panic!("{force} is in range"));
    twin.assert_same(&format!(
        "{force}: set_params has moved nothing, it has armed the next tick"
    ));
    twin.step(1);
    twin
}

/// The gate's own model for `seed` (`prompt.md` §7.1): 2 to 66 nodes over seeds 0..64.
pub(super) fn topology(seed: u32) -> Topology {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the gate's model fits the u32 index space")
}

/// A default session over the gate's model for `seed`.
pub(super) fn session(seed: u32) -> ForceSession {
    ForceSession::new(&topology(seed), LiveParams::default()).expect("the defaults are in range")
}

/// Every position as `f64` bit patterns, `x` then `y` — the comparison that notices a
/// one-ULP move, which `assert_eq!` on `f64` also does but reads as a near-miss.
pub(super) fn bits(session: &ForceSession) -> Vec<u64> {
    session
        .xs()
        .iter()
        .chain(session.ys())
        .map(|v| v.to_bits())
        .collect()
}

/// The stage's own output columns, `f32`, the bytes `layout.force.barnes_hut` hashes.
pub(super) fn as_f32(session: &ForceSession) -> (Vec<f32>, Vec<f32>) {
    (
        session.xs().iter().map(|&v| v as f32).collect(),
        session.ys().iter().map(|&v| v as f32).collect(),
    )
}

/// The digest [`golden::FROZEN`] is taken over: every `x` then every `y`, `f32`
/// little-endian, in node order. The order is part of the constant.
pub(super) fn digest32(xs: &[f32], ys: &[f32]) -> String {
    let mut bytes = Vec::with_capacity((xs.len() + ys.len()) * 4);
    for v in xs.iter().chain(ys) {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    sha256_hex(&bytes)
}

/// The `M1c` command script, verbatim: step 20; pin row 0 at the origin; charge -300;
/// step 30; unpin; reheat 0.5; gravity 0.1; step 50. It touches every verb the session
/// has, in an order where each one has to be read by the next.
pub(super) fn scripted(seed: u32) -> ForceSession {
    let mut session = session(seed);
    session.step(20);
    session
        .pin(NodeRow::new(0), 0.0, 0.0)
        .expect("row 0 exists");
    session
        .set_params(set(&session, "charge", -300.0))
        .expect("-300 is in range");
    session.step(30);
    session.unpin(NodeRow::new(0)).expect("row 0 exists");
    session.reheat(0.5).expect("0.5 is in range");
    session
        .set_params(set(&session, "gravity", 0.1))
        .expect("0.1 is in range");
    session.step(50);
    session
}

/// `session`'s own parameters with one field changed. "Set charge -300" and "set gravity
/// 0.1" are sets on a *live* parameter set, so each keeps what the previous command left —
/// which is why the base is `session.params()` and not the default.
fn set(session: &ForceSession, field: &str, value: f64) -> LiveParams {
    let current = session.params();
    match field {
        "charge" => LiveParams {
            charge: value,
            ..current
        },
        "gravity" => LiveParams {
            gravity: value,
            ..current
        },
        other => panic!("the script sets no {other}"),
    }
}
