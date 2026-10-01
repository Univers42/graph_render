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
        // The 3D arms: the same comparison at three columns, against the reference's own
        // `_spectral_layout_3d` / `_mds_layout_3d` (networkx_layouts.py:249-291), which
        // call `_spectral_component_coordinates(G, 3)` and
        // `_pivot_mds_component_coordinates(G, 3, _MDS_PIVOTS)`.
        //
        // `spectral_3d`'s ceiling is MEASURED and is one power of ten above `spectral`'s
        // for a stated reason, not widened to green: over the same 1000 seeds both arms
        // have the same median (5.2e-8) and the 3D one a slightly heavier tail
        // (p90 3.07e-6 vs 2.58e-6, max 1.0296e-5 at seed 535 vs 7.178e-6 at seed 546).
        // A 3-column span has one more ill-conditioned direction to resolve, so the
        // subspace angle is marginally larger on the same solver pair — it is the same
        // distribution, not a defect, and `docs/measurements/p12-t4a.md` records both
        // numbers. `pivot_mds_3d` measures 3.69e-8, identical to its 2D arm, so it keeps
        // the same 1e-7.
        ("layout.spectral.3d", "spectral_3d", CEILING_SPECTRAL_3D),
        ("layout.mds.pivot.3d", "pivot_mds_3d", 1e-7),
    ],
    line,
};

/// `layout.spectral.3d`'s measured ceiling: 1.0296e-5 over the 1000 gate seeds, rounded
/// up to the next power of ten. Its 2D sibling measures 7.178e-6 against 1e-5; the ratio
/// is the extra ill-conditioned direction in a 3-column span, and the distributions agree
/// at the median (5.2e-8 both). Measured, not widened — see the `ceilings` table.
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
