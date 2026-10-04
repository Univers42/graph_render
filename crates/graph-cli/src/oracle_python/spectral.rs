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
        // The 3-D arms: the same comparison at three columns, against the reference's own
        // `_spectral_component_coordinates(G, 3)` and
        // `_pivot_mds_component_coordinates(G, 3, _MDS_PIVOTS)`
        // (`networkx_layouts.py:133,200`, reached by `_spectral_layout_3d` / `_mds_layout_3d`
        // at `:249,271`).
        //
        // **These ids are develop's, not `p12-t4a`'s** (`docs/decisions/3d-ids.md`): the
        // branch called them `layout.spectral.3d` and `layout.mds.pivot.3d` for the same two
        // run functions, so there is one algorithm here and one id, and the harness keys
        // follow develop's ids rather than the branch's.
        //
        // `spectral_3d`'s ceiling is MEASURED and is one power of ten above `spectral`'s for
        // a stated reason, not widened to green: a 3-column span has one more
        // ill-conditioned direction to resolve on the same solver pair, so the subspace
        // angle is marginally larger on the same distribution. `docs/measurements/p12-3d-oracles.md`
        // records the medians and both worsts. `pivot_mds_3d` measures the same as its 2-D
        // arm and keeps 1e-7.
        ("layout.spectral3d", "spectral_3d", CEILING_SPECTRAL_3D),
        ("layout.mds.pivot3d", "pivot_mds_3d", 1e-7),
    ],
    line,
};

/// `layout.spectral3d`'s measured ceiling: one power of ten above the 2-D arm's 1e-5, and
/// for a stated reason rather than a green row. A `dims`-column span carries `dims - 1`
/// ill-conditioned directions to resolve on the same LOBPCG pair, so at 3 the largest
/// principal angle between our basis and the reference's is marginally larger on the same
/// distribution. Measured over the gate seeds; `docs/measurements/p12-3d-oracles.md` has the
/// numbers, and `harness/oracle-spectral.py`'s own Ponytail says the same thing.
const CEILING_SPECTRAL_3D: f64 = 1e-4;

/// One seed's line. The spectral layouts are closed-form and take no iteration budget,
/// so the emit's `--max-iter` (ForceAtlas2's) does not reach them.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
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
