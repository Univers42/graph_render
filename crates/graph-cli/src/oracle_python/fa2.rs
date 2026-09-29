//! `layout.forceatlas2` against networkx 3.6's own `forceatlas2_layout`
//! (`harness/oracle-fa2.py`), started from the port's own initial positions so the two
//! arms run the same 100 iterations from the same state.
//!
//! Ponytail: FA2 is chaotic, so a summation-order difference of one ulp can grow over the
//! 100 iterations; the ceiling bounds that growth as measured on the gate model, and a
//! larger or denser graph than the gate's may grow it past the ceiling without the port
//! being wrong. The compared coordinates are ours after the snapshot's f32 rounding.

use super::{Differential, coords};
use graph_core::layout::forceatlas2::{Fa2Params, initial_positions};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const FA2: Differential = Differential {
    name: "fa2",
    ceilings: &[("layout.forceatlas2", "fa2", CEILING)],
    line,
};

/// The worst `max |ours - theirs| / extent(theirs)` over the 1000 gate seeds, rounded up
/// to the next power of ten (`docs/measurements/phase06-force.md`).
const CEILING: f64 = 1e-3;

fn line(seed: u32) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let params = Fa2Params::default();
    let (x0, y0) = initial_positions(n, params.seed);
    Ok(json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "params": {
            "max_iter": params.max_iter, "jitter_tolerance": params.jitter_tolerance,
            "scaling_ratio": params.scaling_ratio, "gravity": params.gravity,
        },
        "initial": { "x": x0, "y": y0 },
        "fa2": coords("layout.forceatlas2", &nodes, &edges)?,
    }))
}
