//! `layout.circular.hierarchy` against **SciGraphs' own** `_circular_hierarchy_layout`
//! (`harness/oracle-circular-hierarchy.py`), run in the `ge-python-oracle` image with the
//! `SciGraphs/` submodule mounted so the arm is the reference function itself and not a
//! restatement of it.
//!
//! This is a closed form — no RNG, no iteration, no force — so agreement is a coordinate
//! tolerance and nothing weaker. The gap is the snapshot's own `f32` narrowing plus
//! `np.cos` against `libm::cos`, and the ceiling is the next power of ten above the worst
//! case over 1000 seeds (`docs/measurements/p12-t2.md`).
//!
//! Ponytail: the SciGraphs arm is handed an `nx.Graph`, never an `nx.DiGraph`, because the
//! port reads a directed input undirected — the motor's `Topology` carries directedness per
//! edge and has no whole-graph flag, so SciGraphs' in-degree-0 branch
//! (`hierarchical.py:705-711`) is not ported and is not compared. The gate model is one
//! connected random graph per seed, so a single node, an empty graph and a disconnected one
//! rest on graph-core's own tests, which hold them against the reference's printed answers.

use super::{Differential, coords};
use graph_core::layout::circular::hierarchy::ID;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const CIRCULAR_HIERARCHY: Differential = Differential {
    name: "circular-hierarchy",
    ceilings: &[("layout.circular.hierarchy", "circular-hierarchy", CEILING)],
    line,
};

/// The gap's ceiling: its worst `max |ours - theirs|` over the 1000 gate seeds, rounded up
/// to the next power of ten. A tolerance sitting exactly on the number it was measured from
/// is not one. Ours are the snapshot's `f32` columns, so the floor is that narrowing and
/// nothing else (`docs/measurements/p12-t2.md`).
pub(crate) const CEILING: f64 = 1e-6;

/// SciGraphs' `apply_graph_layout(scale=5.0)` default (`dispatcher.py:14`), restated in the
/// fixture so the arm is handed the same `scale` the module hard-codes. A layout that took
/// no budget ignores `--max-iter`, as `closed_form.rs`'s does.
const SCALE: f64 = 5.0;

/// One seed's line. The layout takes no iteration budget, so `--max-iter` is ignored.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    Ok(json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "scale": SCALE,
        "circular-hierarchy": coords(ID, &nodes, &edges)?,
    }))
}
