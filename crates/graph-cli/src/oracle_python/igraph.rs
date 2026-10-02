//! The six igraph-family force layouts against python-igraph 0.11.9
//! (`harness/oracle-igraph.py`), on a layout-quality metric rather than coordinates.
//!
//! igraph draws its own randomness for most of these (LGL takes no start at all), and
//! all six are iterative, so a coordinate comparison would gate on chaos
//! (`prompts/RESUME.md` item 4). The metric is normalised stress against graph distance,
//! after the optimal uniform scale, and the verdict is `ours / igraph`: how many times
//! worse than the reference our layout is on the same graph.
//!
//! A layout that is not yet registered writes no `ours` column, so the harness compares
//! nothing for it and [`super::judge`] fails it (no compared case): NOT-RUN, never green.
//!
//! Ponytail: stress rewards a layout for matching graph distance, which FR and Graphopt do
//! not try to do (they balance forces), so their ratio is a quality floor, not a
//! disagreement measure. Direction: a correct force layout can read worse than igraph's
//! here, and a stress-optimal but ugly layout reads better. Ceilings are measured
//! worst cases rounded up (`docs/decisions/layouts-igraph.md`).

use super::{Differential, columns_3d, coords};
use graph_core::layout::forceatlas2::initial_positions;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, registry, seeded_model};
use serde_json::{Value, json};

// Measured worst `ours / igraph` over 100 seeds (2026-09-29), rounded up to the next power
// of ten: FR 1.30, KK 1.35, DrL 3.98, LGL 2.13, Davidson-Harel 51.9, Graphopt 15.4.
// Ponytail: a power of ten is loose (DH could regress 2x unseen); it catches breakage, not drift.
const CEILING_TIGHT: f64 = 10.0;
const CEILING_LOOSE: f64 = 100.0;

pub const IGRAPH: Differential = Differential {
    name: "igraph",
    ceilings: &[
        (
            "layout.force.fruchterman_reingold",
            "fruchterman_reingold",
            CEILING_TIGHT,
        ),
        ("layout.force.kamada_kawai", "kamada_kawai", CEILING_TIGHT),
        // The two `_3d` rows are the dimension SciGraphs actually calls
        // (`igraph_layouts.py:74`, `:99`), and they carry the **same** ceilings as their 2D
        // siblings rather than a newly measured pair. Stated, because "the same ceiling" is a
        // choice and not a measurement: the stress ratio is scale-invariant, the metric is
        // unchanged, and the 3D solve differs from the 2D one only in its linear algebra, so
        // the 2D worst case is the honest prior. They are held to it as a floor that catches
        // breakage; a measured 3D worst would be another job's number to re-pin. The 2D rows
        // stay in the list and stay gated — this is an addition, never a replacement.
        (
            "layout.force.fruchterman_reingold_3d",
            "fruchterman_reingold_3d",
            CEILING_TIGHT,
        ),
        (
            "layout.force.kamada_kawai_3d",
            "kamada_kawai_3d",
            CEILING_TIGHT,
        ),
        ("layout.force.drl", "drl", CEILING_TIGHT),
        ("layout.force.lgl", "lgl", CEILING_TIGHT),
        (
            "layout.force.davidson_harel",
            "davidson_harel",
            CEILING_LOOSE,
        ),
        ("layout.force.graphopt", "graphopt", CEILING_LOOSE),
    ],
    line,
};

/// The seed's gate model, the start positions handed to igraph as `seed`, and the
/// coordinates of each registered layout.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let (x0, y0) = initial_positions(n, seed);
    let mut out = json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "initial": { "x": x0, "y": y0 }, "ours": {},
    });
    for &(id, key, _) in IGRAPH.ceilings {
        if registry::find(id).is_some() {
            let layout = registry::find(id).ok_or_else(|| format!("{id}: gone"))?;
            let run =
                graph_core::run_with(&nodes, &edges, id, layout.run).map_err(|e| e.to_string())?;
            let parts = run.snapshot.parts();
            let value = if id.ends_with("_3d") {
                // A 3D layout's third column is read off the **snapshot**, not the geometry: a
                // z that never reached the wire would otherwise be scored as a planar drawing
                // against a 3D reference, and the ratio would be a number about nothing.
                columns_3d(id, &parts.nodes, parts.z.as_deref())?
            } else {
                coords(id, &nodes, &edges)?
            };
            // The id rides with the columns so the harness can tell a `_3d` row that lost its
            // z on the way here from one that never had one.
            out["ours"][key] = json!({
                "id": id, "x": value["x"], "y": value["y"], "z": value["z"],
            });
        }
    }
    Ok(out)
}
