//! `layout.twopi` against **Graphviz's own** `twopi`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-twopi.py`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` the other three differentials emit — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over, and the engine's output is reproducible by the `cmp` the ADR
//! records.
//!
//! **Unlike the other three, this arm's coordinates are not a tolerance against a
//! re-derivation of the same formula.** They are another implementation's, in points, and
//! the ceiling is [`CEILING`], the next power of ten above the worst gap measured over the
//! 1000 gate seeds (`docs/measurements/p13-gv1.md`). `-Gstart` is inert for this engine —
//! measured, not assumed — so the gap is an algorithmic difference or nothing, never seed
//! drift.
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, so at a drawing
//! 17 inches across no comparison can be tighter than about 7e-2 points whatever the
//! arithmetic. The ceiling reflects that, not a disagreement; the six analytically
//! determined small cases are compared byte for byte in both arms — Graphviz's here and our
//! own node by node in graph-core's tests — where a tolerance is weaker than the truth.

use super::{Differential, coords};
use graph_core::layout::radial::twopi;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const TWOPI: Differential = Differential {
    name: "twopi",
    ceilings: &[("layout.twopi", "twopi", CEILING)],
    line,
};

/// The twopi ceiling: the worst `max |ours - theirs|` in points over the 1000 gate seeds is
/// **7.10e-2**, rounded up to the next power of ten.
///
/// That figure is the oracle's own printed resolution and not a disagreement: `-Tplain`
/// writes five significant digits, so at the largest gate drawing — 16.95 inches across,
/// 1220 points — one printed digit is 0.001 inch, which is 0.072 points. The per-seed gaps
/// track the drawing's extent to within a factor of 1.02 across the whole sweep, the median
/// is 4.8e-2, and 846 of the 1000 seeds land above 1e-2 (`docs/measurements/p13-gv1.md`).
/// A tighter ceiling would be a claim about `-Tplain`'s formatter, not about the layout.
pub(crate) const CEILING: f64 = 1e-1;

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
    out["twopi"] = coords(twopi::ID, &nodes, &edges)?;
    Ok(out)
}
