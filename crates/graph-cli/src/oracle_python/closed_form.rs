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
    ],
    line,
};

/// The ring's ceiling: its worst `max |ours - theirs|` over the 1000 gate seeds is 2.65e-7
/// (networkx narrows the angle to `f32`), rounded up to the next power of ten. Spiral and
/// bipartite measure 2.98e-8, the snapshot's `f32` floor, so theirs is 1e-7
/// (`docs/measurements/closed-form-oracle.md`).
pub(crate) const CEILING: f64 = 1e-6;

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
