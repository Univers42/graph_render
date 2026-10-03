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

use super::{Differential, coords, points};
use graph_core::layout::forceatlas2::initial_positions;
use graph_core::{
    REFERENCE_DEGREE, gate_node_count, index_model, registry, run_with, seeded_model,
};
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

/// The 3D arms, in a differential of their own.
///
/// **A second `Differential`, not a second `ceilings` list on this one**, because the two
/// arms need different fixtures: igraph at `dim = 3` takes a `seed` of `[x, y, z]` per node
/// (`igraph_layouts.py:301`), and the stress metric at 3 dimensions is a different
/// measurement from the 2D one rather than the same number in a bigger box. Sharing the
/// key space would let a 2D result be read as a 3D verdict.
pub const IGRAPH_3D: Differential = Differential {
    name: "igraph3d",
    ceilings: &[
        (
            "layout.force.fruchterman_reingold.3d",
            "fruchterman_reingold",
            CEILING_3D_TIGHT,
        ),
        (
            "layout.force.kamada_kawai.3d",
            "kamada_kawai",
            CEILING_3D_KK,
        ),
        ("layout.force.drl.3d", "drl", CEILING_3D_TIGHT),
    ],
    line: line_3d,
};

/// The 3D ceilings, measured over the 1000 gate seeds and rounded up to the next power of
/// ten above the worst `ours / igraph` (`docs/measurements/p12-t4b.md`): FR 1.19, DrL 1.26,
/// KK 18.9 — so 1e1 for the first two and 1e2 for KK. Held apart from the 2D
/// [`CEILING_TIGHT`]: a 3D stress ratio has no reason to equal a 2D one, and reusing the 2D
/// number would be asserting a measurement nobody took.
///
/// **KK's 3D number is 18.9, and that is not a rounding accident.** The 2D arm measures
/// 1.35 at the same budget. The cause is stated rather than smoothed over: a Newton step
/// solves a 3x3 block here where igraph's own 3D descent takes a different route, and
/// KK's stress after descent is a local minimum, so the two settle in different basins from
/// the same start. The ratio is a quality floor on a local method, not a disagreement
/// measure, and 1e2 is the honest ceiling for it — narrowing it to 1e1 would be picking a
/// number to make a row green, and the row's status stays `implemented` either way.
const CEILING_3D_TIGHT: f64 = 10.0;
const CEILING_3D_KK: f64 = 100.0;

/// The 3D fixture line: the same model, the same 2D-started 3D seeds, and only the arms
/// that answer in three coordinates.
fn line_3d(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let start = initial_positions(n, seed, 3);
    let mut out = json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "initial_3d": {
            "x": column(&start, 0), "y": column(&start, 1), "z": column(&start, 2),
        },
        "ours": {},
    });
    for &(id, key, _) in IGRAPH_3D.ceilings {
        if registry::find(id).is_some() {
            out["ours"][key] = space_coords(id, &nodes, &edges)?;
        }
    }
    let _ = topology;
    Ok(out)
}

/// The seed's gate model, the start positions handed to igraph as `seed`, and the
/// coordinates of each registered layout.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let start = initial_positions(n, seed, 2);
    let start_3d = initial_positions(n, seed, 3);
    let mut out = json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "initial": { "x": column(&start, 0), "y": column(&start, 1) },
        "initial_3d": {
            "x": column(&start_3d, 0), "y": column(&start_3d, 1),
            "z": column(&start_3d, 2),
        },
        "ours": {}, "ours_3d": {},
    });
    for &(id, key, _) in IGRAPH.ceilings {
        if registry::find(id).is_some() {
            out["ours"][key] = coords(id, &nodes, &edges)?;
        }
    }
    for &(id, key, _) in IGRAPH_3D.ceilings {
        if registry::find(id).is_some() {
            out["ours_3d"][key] = space_coords(id, &nodes, &edges)?;
        }
    }
    Ok(out)
}

fn column(start: &[[f64; 3]], axis: usize) -> Value {
    json!(start.iter().map(|p| p[axis]).collect::<Vec<_>>())
}

/// A 3D layout's three columns, as the harness needs them.
///
/// The refusal is the point: a 3D id whose run answered without a z column is a 2D
/// picture under a 3D name, and scoring it against a 3D igraph would compare two
/// different things and call it agreement.
fn space_coords(
    id: &str,
    nodes: &[graph_core::NodeRecord],
    edges: &[graph_core::EdgeRecord],
) -> Result<Value, String> {
    let layout = registry::find(id).ok_or_else(|| format!("{id}: not registered"))?;
    let run = run_with(nodes, edges, layout.id, layout.run).map_err(|e| e.to_string())?;
    let mut out = points(id, &run.snapshot.parts().nodes)?;
    match &run.snapshot.parts().z {
        Some(z) => {
            out["z"] = json!(z);
            Ok(out)
        }
        None => Err(format!("{id}: expected a z column, got none")),
    }
}
