//! The native arm of the force gate: one live session over the gate's own model for one seed,
//! stepped [`TICKS`](super::TICKS) times, and the two position columns as the bytes both arms
//! hash.
//!
//! **The model is the gate's, at the compiled-in degree and node count** — not
//! `setting.reference_degree` and `setting.extra_nodes`. The wasm arm cannot draw any other one:
//! its model comes from `gm_seed_ingest`, a documented gate-only export whose document is fixed.
//! Building a different model here would make the two arms compare two different questions, so
//! the settings that would do it are refused by
//! [`super::refuse_a_control_that_cannot_bite`] instead of being read.
//!
//! The hash is over `f64` bits, little-endian, `x` then `y`: the same bytes the wasm arm's
//! `Float64Array` reads out of the module's memory, which is what makes the two digests
//! directly comparable.

use super::TICKS;
use crate::hashgate::knob::Setting;
use graph_core::layout::force::ForceSession;
use graph_core::{REFERENCE_DEGREE, Topology, gate_node_count, index_model, seeded_model};

/// The gate's model for `seed`, indexed: the same nodes and edges `gm_seed_ingest` writes as
/// ingest JSON and `gm_build` reads back, so both arms simulate the same graph.
pub fn topology(seed: u32) -> Result<Topology, String> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    index_model(&nodes, &edges).map_err(|e| e.to_string())
}

/// A live session over that model at `setting`'s live parameters.
pub fn session(seed: u32, setting: &Setting) -> Result<ForceSession, String> {
    ForceSession::new(&topology(seed)?, setting.live_force_params())
        .map_err(|e| format!("the force session refused the model: {e}"))
}

/// The bytes this gate hashes for `seed`: [`TICKS`] ticks of a live session over the gate's
/// model, then `x` followed by `y`, one `f64` per node, in row order.
///
/// `to_le_bytes` rather than a `bytemuck`-style cast so the byte order is stated here and
/// matches what the wasm arm's typed-array read produces on the other side of the same
/// comparison — the two are different languages, and the bytes are what they agree on.
pub fn positions(seed: u32, setting: &Setting) -> Result<Vec<u8>, String> {
    let mut session = session(seed, setting)?;
    session.step(TICKS);
    Ok(columns(&session))
}

/// The two position columns as the bytes every arm hashes: `x` then `y`, one `f64` per node,
/// in row order, little-endian.
///
/// **The one writer of these bytes for the whole gate**, so the seed arm and the stream arm
/// cannot hash two layouts that look alike: they call this, and `stream-arm.mjs` reproduces
/// it through a `Float64Array` read on the other side of the same comparison.
pub fn columns(session: &ForceSession) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 * (session.xs().len() + session.ys().len()));
    for column in [session.xs(), session.ys()] {
        for value in column {
            out.extend_from_slice(&value.to_le_bytes());
        }
    }
    out
}
