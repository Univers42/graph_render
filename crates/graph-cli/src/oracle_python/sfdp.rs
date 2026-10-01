//! The `layout.force.sfdp` differential: our multilevel spring-electrical layout against
//! Graphviz's own.
//!
//! Shape: `super::twopi`'s, down to the only two things this file hardcodes — the capability
//! id it reads out of the registry and the JSON key the harness reads back. The harness is
//! `harness/oracle-graphviz.py --differential`, which owns the metric, the rescale and the
//! closed case; nothing here recomputes any of it.

use super::{Differential, coords};
use graph_core::layout::graphviz::sfdp;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

/// The measured worst gap over the 1000-seed sweep, in points, on the metric
/// `harness/oracle-graphviz.py` applies (largest absolute node-coordinate difference after both
/// arms are rescaled onto one bounding box). See `docs/measurements/p13-gv2-sfdp.md`.
///
/// This number is **not** a shortfall to be closed by writing a better port: this engine is
/// seed-sensitive, and the oracle compared against itself at `-Gstart` 7 rather than 1 differs
/// by up to 4.81e+2 points on this same metric — a larger gap than our own arm shows. The
/// ceiling is therefore set by the oracle's
/// own seed-to-seed spread, and the row is `Status::Implemented` rather than gated.
pub(crate) const CEILING: f64 = 1e3;

pub const SFDP: Differential = Differential {
    name: "sfdp",
    ceilings: &[("layout.force.sfdp", "sfdp", CEILING)],
    line,
};

fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out["sfdp"] = coords(sfdp::ID, &nodes, &edges)?;
    Ok(out)
}
