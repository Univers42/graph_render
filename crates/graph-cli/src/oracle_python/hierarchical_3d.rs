//! `layout.hierarchical3d` against **SciGraphs' own** `_hierarchical_layout_3d`
//! (`harness/oracle-hierarchical-3d.py`), run in the `ge-python-oracle` image with the
//! `SciGraphs/` submodule mounted so the arm is the reference function itself and not a
//! restatement of it.
//!
//! **Its own arm file, not one shared with the three graph-free placements.** It is the
//! only one of the five that reads a graph, so it is compared over *shapes* — levels, disk
//! radii, ring splits — rather than over node counts, and the fixture it needs is the
//! gate's own model with its edges. Folding it into `oracle-basic-3d.py` would put a
//! graph-reading layout and two graph-free ones behind one `--function` selector whose
//! inputs are not the same.
//!
//! This is a closed form — no RNG, no iteration, no force — so agreement is a coordinate
//! tolerance and nothing weaker. The gap is the snapshot's own `f32` narrowing plus
//! `np.cos` against `libm::cos`, with one further contributor that is a fact about the
//! reference rather than about the port: `_disk_positions` rounds with **ties to even**
//! (`hierarchical.py:97`, `:99`), so at a level whose `take` lands on an exact half the two
//! roundings differ by a whole point and every node after it moves. The port implements
//! ties-to-even for that reason (`layout/hierarchical_3d/disk.rs`'s `half_to_even`), and
//! the arm therefore has to use the reference's own rounding too — which it does, by being
//! the reference.
//!
//! Ponytail: the arm is handed an `nx.Graph`, never an `nx.DiGraph`. The port reads a
//! directed input undirected, because the motor's `Topology` carries directedness per edge
//! and has no whole-graph flag, so SciGraphs' in-degree-0 roots branch
//! (`hierarchical.py:127-128`) is not ported and is not compared — the same departure the
//! sibling `oracle-circular-hierarchy.py` states. The ceiling below is **declared, not
//! measured on this tree** (see `basic_3d.rs`'s module doc for the same clause): one power
//! of ten above the `f32` floor, which is what the only expected difference amounts to.

use super::{Differential, columns_3d};
use graph_core::layout::hierarchical_3d::ID;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const HIERARCHICAL_3D: Differential = Differential {
    name: "hierarchical-3d",
    ceilings: &[(ID, "hierarchical-3d", CEILING)],
    line,
};

/// The coordinate ceiling: one power of ten above the snapshot's `f32` narrowing, which is
/// the whole of the expected gap. Declared rather than measured on this tree — see the
/// module doc.
pub(crate) const CEILING: f64 = 1e-6;

/// SciGraphs' `apply_graph_layout(scale=5.0)` default (`dispatcher.py:14`), restated so the
/// arm is handed the same `scale` `hierarchical_3d::SCALE` hard-codes. The layout takes no
/// iteration budget, so `--max-iter` is ignored.
const SCALE: f64 = 5.0;

/// One seed's line: the gate's own model, its edges, and the layout's three columns.
///
/// Unlike the three graph-free arms, the edges are load-bearing here — they are what makes
/// the BFS levels — so both endpoint columns are emitted and the arm rebuilds the same
/// undirected simple graph the port reads.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let layout = graph_core::registry::find(ID).ok_or_else(|| format!("{ID}: gone"))?;
    let run = graph_core::run_with(&nodes, &edges, ID, layout.run).map_err(|e| e.to_string())?;
    let parts = run.snapshot.parts();
    Ok(json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "scale": SCALE,
        "hierarchical-3d": columns_3d(ID, &parts.nodes, parts.z.as_deref())?,
    }))
}
