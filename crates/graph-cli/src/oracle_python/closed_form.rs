//! `layout.circular.ring`, `layout.spiral` and `layout.bipartite` against networkx 3.6's
//! own `circular_layout`, `spiral_layout` and `bipartite_layout`
//! (`harness/oracle-closed-form.py`). The three are closed forms, so agreement is
//! coordinates within [`CEILING`], not a statistic.
//!
//! **The 3-D arms and why two of them needed the SciGraphs package rather than networkx.**
//! `layout.random.3d`, `layout.basic3d.spiral` and `layout.bipartite_3d` are the same three
//! algorithms at three columns, and their oracles are the ones that exist: networkx
//! `random_layout` does take `dim=3`, while `spiral_layout` REFUSES `dim=3` (it is planar by
//! construction) and `bipartite_layout` has no `dim` at all. So the two coordinate arms are
//! compared against SciGraphs' own `_spiral_layout_3d` (`basic.py:36-63`) and
//! `_bipartite_layout_3d` (`hierarchical.py:213-242`) at the dispatcher's `scale = 5.0`,
//! and `layout.random.3d` against the shape of its `_random_layout` (`basic.py:5-9`).
//! Measured ceilings and the arms' own limitations: `docs/measurements/p12-3d-oracles.md`.
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
        ("layout.random.3d", "random_3d", CEILING_3D_RANDOM),
        ("layout.basic3d.spiral", "spiral_3d", CEILING_3D_COORDS),
        ("layout.bipartite_3d", "bipartite_3d", CEILING_3D_COORDS),
    ],
    line,
};

/// The ring's ceiling: its worst `max |ours - theirs|` over the 1000 gate seeds is 2.65e-7
/// (networkx narrows the angle to `f32`), rounded up to the next power of ten. Spiral and
/// bipartite measure 2.98e-8, the snapshot's `f32` floor, so theirs is 1e-7
/// (`docs/measurements/closed-form-oracle.md`).
pub(crate) const CEILING: f64 = 1e-6;

/// The 3-D coordinate arms' ceiling, one number for both, and **measured, never widened to
/// green a row**.
///
/// It is 1e-6 and not the 1e-7 the two 2-D siblings carry, for one measurable reason: these
/// two ids draw at the 3-D dispatcher's `scale = 5.0`, so their coordinates run to about
/// +/-5 where the 2-D arms run to +/-1, and the snapshot's `f32` floor is *relative*. Over
/// the 1000 gate seeds, measured on this tree
/// (`docs/measurements/p12-3d-oracles.md`): `spiral_3d` 2.384e-7, `bipartite_3d` 1.192e-7.
/// Both are 2^-22 and 2^-23, i.e. the `f32` spacing at that magnitude and nothing else, and
/// both round up to the same power of ten. The 2-D arms measure 2.980e-8 at +/-1, the same
/// figure scaled down by the same five.
///
/// This is also the ceiling `oracle-basic-3d` already carries for these two ids over the same
/// 1000 seeds (`registry/three_d/spiral3d.rs:19`), which is the cross-check that the number
/// is the layout's and not this arm's: two independent harness paths, one figure.
pub(crate) const CEILING_3D_COORDS: f64 = 1e-6;

/// **`layout.random.3d`'s ceiling, and it is not a coordinate tolerance.** Our stream is
/// `Mulberry32` at a fixed seed and the reference's is numpy's Mersenne Twister off
/// `get_layout_seed()`, so no coordinate can ever match and the arm compares the
/// DISTRIBUTION instead: the worst per-axis error in the sample mean (target 1/2) and in
/// the sample variance (target 1/12).
///
/// That metric's floor is the **smallest graph in the sweep**, not our arithmetic. Measured
/// over the 1000 gate seeds the worst is 0.30300, and it happens at `n = 2`, where the
/// expected `|sample mean - 1/2|` for two uniform draws is 0.204 — the same order, and the
/// figure a two-point sample can reach at worst is 0.5. It falls as `1/sqrt(n)` (0.0384 over
/// the cases with `n >= 100`), which is the signature of sampling noise rather than of a
/// skewed stream.
///
/// So the ceiling is the measured worst rounded up to the next half — 0.5, one significant
/// figure above it, so the row still bites if the stream ever skews — and NOT the 1e-6 the
/// two coordinate arms carry. `docs/measurements/p12-3d-oracles.md` records the measured
/// worst and the `n` it happened at; the ceiling is measured and never widened to green.
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
