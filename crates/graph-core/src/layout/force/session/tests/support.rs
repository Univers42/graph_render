//! What the five cases share: the gate's own synthetic topology, a default session over
//! it, the byte-level views the assertions compare, and the `M1c` command script.

use super::digest::sha256_hex;
use crate::index::{Topology, index_model};
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};
use crate::stage::{gate_node_count, seeded_model};
use crate::weights::REFERENCE_DEGREE;

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
