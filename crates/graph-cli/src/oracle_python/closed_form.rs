//! `layout.circular.ring`, `layout.spiral` and `layout.bipartite` against networkx 3.6's
//! own `circular_layout`, `spiral_layout` and `bipartite_layout`
//! (`harness/oracle-closed-form.py`). The three are closed forms, so agreement is
//! coordinates within [`CEILING`], not a statistic.
//!
//! Ponytail: the bipartite comparison takes the node sets from our own output (the lower
//! column is the first set) and hands them to networkx through `nodes=`, so it checks the
//! geometry given a partition, not the partition rule, and not the order inside a column
//! (networkx's follows Python set iteration, ours the partition's own order): columns are
//! compared as sorted values. A gate graph is one connected random graph of 2 to 601
//! nodes, so a single node, an empty graph and a disconnected one rest on graph-core's
//! own tests. Ours are the snapshot's `f32` columns, a floor near 1e-7.

use super::{Differential, coords};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const CLOSED_FORM: Differential = Differential {
    name: "closed-form",
    ceilings: &[
        ("layout.circular.ring", "ring", CEILING),
        ("layout.spiral", "spiral", 1e-7),
        ("layout.bipartite", "bipartite", 1e-7),
        // The 3D arms of `docs/measurements/p12-t4a.md`. Their oracles are the ones that
        // exist: networkx `random_layout` does take dim=3, while `spiral_layout` REFUSES
        // dim=3 and `bipartite_layout` has no dim at all, so those two are compared against
        // SciGraphs' own `_spiral_layout_3d` / `_bipartite_layout_3d` (the job's Q1 answer).
        ("layout.random.3d", "random_3d", CEILING_3D_RANDOM),
        ("layout.spiral.3d", "spiral_3d", CEILING_3D_COORDS),
        ("layout.bipartite.3d", "bipartite_3d", CEILING_3D_COORDS),
    ],
    line,
};

/// The ring's ceiling: its worst `max |ours - theirs|` over the 1000 gate seeds is 2.65e-7
/// (networkx narrows the angle to `f32`), rounded up to the next power of ten. Spiral and
/// bipartite measure 2.98e-8, the snapshot's `f32` floor, so theirs is 1e-7
/// (`docs/measurements/closed-form-oracle.md`).
pub(crate) const CEILING: f64 = 1e-6;

/// The 3D arms' coordinate ceiling, measured over the 1000 gate seeds on 2026-10-01:
/// `spiral_3d` 2.9802e-8 and `bipartite_3d` 2.9798e-8, both the snapshot's `f32` floor and
/// both the same figure their 2D arms measure (2.9802e-8 / 2.9801e-8). Rounded up to 1e-7
/// for the same reason the 2D rows are (`docs/measurements/p12-t4a.md`).
pub(crate) const CEILING_3D_COORDS: f64 = 1e-7;

/// **`layout.random.3d`'s ceiling, and it is not a coordinate tolerance.** Our stream is
/// Mulberry32 and the reference's is numpy's Mersenne Twister, so no coordinate can ever
/// match and the arm compares the DISTRIBUTION instead: the worst per-axis error in the
/// sample mean (target 1/2) and variance (target 1/12).
///
/// That metric's floor is the **smallest graph in the sweep**, not our arithmetic: at
/// `n = 2` the measured worst is 0.3030, and the expected `|sample mean - 1/2|` for two
/// uniform draws is 0.204 — the same order. It falls as `1/sqrt(n)` (0.0384 over the 804
/// cases with `n >= 100`), which is the signature of sampling noise rather than a bias.
///
/// So the ceiling is the measured worst rounded up — 0.3030 -> **0.5**, one significant
/// figure above it, so the row still bites if the stream ever skews — and NOT the 1e-7 the
/// two coordinate arms carry. Recorded in `docs/measurements/p12-t4a.md`; the ceiling is
/// measured and never widened to green a row.
pub(crate) const CEILING_3D_RANDOM: f64 = 0.5;

/// One seed's line. The layouts take no iteration budget, so `--max-iter` is ignored.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    for &(id, key, _) in CLOSED_FORM.ceilings {
        out[key] = coords(id, &nodes, &edges)?;
    }
    Ok(out)
}
