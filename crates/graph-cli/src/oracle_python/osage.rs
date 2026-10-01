//! `layout.packing.osage` against **Graphviz's own** `osage`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py --differential`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` every other differential emits — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over, and the engine's output is reproducible by the `cmp` the ADR
//! records.
//!
//! **Unlike a re-derivation of the same formula, this arm's coordinates are another
//! implementation's.** osage never reads an edge, so its answer is a pure function of the
//! node count *and of the rendered node labels*, which is more than it looks: see
//! [`CEILING`], which records a measured disagreement and names both causes. `-Gstart` is
//! inert for this engine — measured, not assumed.
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, so on the seeds
//! where the two arms agree no comparison can be tighter than one printed digit. The six
//! analytically determined small cases are compared byte for byte in both arms —
//! Graphviz's here, our own node by node in graph-core's tests — where a tolerance is
//! weaker than the truth.

use super::{Differential, coords};
use graph_core::layout::graphviz::osage;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const OSAGE: Differential = Differential {
    name: "osage",
    ceilings: &[("layout.packing.osage", "osage", CEILING)],
    line,
};

/// The osage ceiling: **1e4 points**, the next power of ten above the worst gap measured
/// over the 1000 gate seeds, which is **1.785e3** at seed 584 (n = 586).
///
/// **This ceiling records a disagreement; it does not excuse one.** The measurement says
/// the two arms agree to within the oracle's own printed quantum — 3.6e-3 points — on
/// exactly the seeds where every node box ties (n ≤ 10, 18 of 1000 seeds, the closed
/// cases), and disagree by 122 to 1785 points on the other 982, where they are two
/// different drawings. Two named causes, both outside the motor, neither an iteration or a
/// tolerance (`docs/measurements/p13-gv1-osage.md`):
///
/// 1. **Graphviz sizes a node's box from its rendered label.** `-Tplain` reports 54 points
///    for `n0`..`n9` and 57.942 for `n10`..`n99` — a three-character label grows the box
///    past the 0.75 inch minimum — so from n = 11 the per-column cell width stops being a
///    constant. That width is a font metric of Graphviz's own text layout.
/// 2. **`arrayRects` sorts the boxes by `width + height` with `qsort`, which is not
///    stable**, so which node lands in which cell among tied boxes is glibc's choice. For
///    n ≤ 10 every box ties and the reference's own output is in declaration order; from
///    n = 11 the wide boxes sort first and move whole rows.
///
/// The row stays `Status::Implemented` for the same reason `layout.twopi`'s does: an
/// agreement this narrow does not earn `gated`.
pub(crate) const CEILING: f64 = 1e4;

/// One seed's line: the gate's model, its bare graph structure for the harness to write
/// DOT from, and our own coordinates. The layout is closed form, so the emit's `--max-iter`
/// (ForceAtlas2's) does not reach it.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out["osage"] = coords(osage::ID, &nodes, &edges)?;
    Ok(out)
}
