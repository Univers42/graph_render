//! `layout.spectral` and `layout.mds.pivot` against SciGraphs' own networkx/scipy
//! implementations (`harness/oracle-spectral.py`).
//!
//! Ponytail: the spectral metric is a subspace angle, so it cannot see a reflection or a
//! rotation inside the 2D span; degenerate eigenspaces rest on the closed-form spectra in
//! graph-core's own tests.

use super::{Differential, coords};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const SPECTRAL: Differential = Differential {
    name: "spectral",
    ceilings: &[
        ("layout.spectral", "spectral", 1e-5),
        ("layout.mds.pivot", "pivot_mds", 1e-7),
    ],
    line,
};

fn line(seed: u32) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    for &(id, key, _) in SPECTRAL.ceilings {
        out[key] = coords(id, &nodes, &edges)?;
    }
    Ok(out)
}
