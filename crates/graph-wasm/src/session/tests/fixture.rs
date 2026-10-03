//! The shared setup every test in this module's tree uses: the gate's own model, a live
//! session over it, and one column as the wire reads it.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because these helpers are what
//! the other two files have in common — `bits.rs` needs a session and both columns,
//! `refusals.rs` needs a session and the bit comparison.

use super::super::{Engine, create, reset, with};
use graph_core::layout::force::LiveParams;
use graph_core::{Topology, index_model, seeded_model};

/// The gate's own synthetic model for `seed`, indexed: the same fixture every hash-gate arm
/// builds, so a divergence found in a test here is one the gate would also see.
pub fn model(seed: u32, nodes: u32) -> Topology {
    let (nodes, edges) = seeded_model(seed, nodes, graph_core::REFERENCE_DEGREE);
    index_model(&nodes, &edges).expect("the gate's model fits")
}

/// A live session over the gate's model at the default parameters, after clearing the table —
/// every test in this tree that wants a session wants a *fresh* one, since the table is
/// process-wide across a test binary.
pub fn session_over(nodes: u32) -> u32 {
    reset();
    create(&model(1, nodes), params(), Engine::BarnesHut).expect("a default session is in range")
}

/// The default parameters: the frozen force set, which is what a session created with no
/// parameter buffer gets.
pub fn params() -> LiveParams {
    LiveParams::default()
}

/// One column as the wire reads it — one `f64` per node.
///
/// **The session's own slice, not a dereference of the address.** The address export refuses
/// a host address past `u32` with `IndexOutOfRange`, so a native test cannot follow it; reading the
/// address half on wasm32 is the force gate's wasm arm's job
/// (`crates/graph-cli/src/forcecheck/arm.mjs`), which hashes exactly these two columns through
/// the wire's `(ptr, len)` on every seed.
pub fn wire_of(id: u32, axis: u32) -> Vec<f64> {
    with(id, |session| {
        Ok(match axis {
            0 => session.xs().to_vec(),
            _ => session.ys().to_vec(),
        })
    })
    .expect("a live session")
}

/// The bits of a column, so a comparison cannot pass on `0.0 == -0.0` — which is the whole
/// reason these tests compare bits rather than values.
pub fn bits(values: &[f64]) -> Vec<u64> {
    values.iter().map(|value| value.to_bits()).collect()
}
