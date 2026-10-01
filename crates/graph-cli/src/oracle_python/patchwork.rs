//! `layout.treemap.patchwork` against **Graphviz's own** `patchwork`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py --differential`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` every other differential emits — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over.
//!
//! **Unlike a re-derivation of the same formula, this arm's coordinates are another
//! implementation's.** patchwork is a squarified treemap over the cluster tree: every node
//! is a square whose area comes from the `area` attribute, the field is `sqrt(1000 * n)`
//! points across, and no edge is read at all. `-Gstart` is **INERT** here — measured over
//! all 1000 seeds at start 1, 7 and 99, byte-identical — so a gap is an algorithmic
//! difference or nothing, never seed drift.
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, and Graphviz's
//! own drawing extent differs from the closed form by up to 3.62e-3 points (measured over
//! n = 1..100), so byte-exact agreement against the oracle's *text* is not reachable and is
//! not claimed. The five analytically determined small cases are instead pinned exactly
//! against the closed form in `crates/graph-core/src/layout/graphviz/patchwork/tests.rs`,
//! and Graphviz's own printed lines for the same five graphs are recorded in
//! `docs/measurements/p13-gv1-patchwork.md`.

use super::{Differential, coords};
use graph_core::layout::graphviz::patchwork;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const PATCHWORK: Differential = Differential {
    name: "patchwork",
    ceilings: &[("layout.treemap.patchwork", "patchwork", CEILING)],
    line,
};

/// The patchwork ceiling: **1e-1 points**, the next power of ten above the worst gap
/// measured over the 1000 gate seeds, which is **6.613e-2** at seed 569 (n = 571).
///
/// **That figure is the oracle's own printed resolution, not a disagreement.** `-Tplain`
/// writes five significant digits, so at the largest gate drawing — 10.34 inches across,
/// 744 points — one printed digit is 0.001 inch, which is 0.072 points. The per-seed gaps
/// track the drawing's extent to within a factor of 1.07, the median is 5.054e-3, and 4 of
/// the 1000 seeds are exact. The residual is measured directly on isolated graphs too:
/// Graphviz's own reported extent sits up to 3.618e-3 points off the exact field
/// `sqrt(1000 * n)` over n = 1..100, which is that same fifth digit moving. A tighter
/// ceiling would be a claim about `-Tplain`'s formatter, not about the layout.
///
/// The row stays `Status::Implemented` rather than `gated`: a differential over another
/// implementation's coordinates is a tolerance by nature, and `layout.twopi`'s row and
/// `layout.packing.osage`'s are `implemented` for the same reason. The full measurement,
/// including the five closed cases compared node by node, is in
/// `docs/measurements/p13-gv1-patchwork.md`.
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
    out["patchwork"] = coords(patchwork::ID, &nodes, &edges)?;
    Ok(out)
}
