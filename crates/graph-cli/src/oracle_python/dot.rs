//! `layout.dag.dot` against **Graphviz's own** `dot`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py --differential`).
//!
//! Shape: [`twopi`](../twopi/index.html)'s — the gate's own model, `source`/`target` for the
//! harness to write DOT from, and our own coordinates under the `"dot"` key. The layout is
//! closed form in its own passes, so the emit's `--max-iter` (ForceAtlas2's) does not reach it.
//!
//! **No `box` column, unlike [`osage`](../osage/index.html).** `dot` sizes a node from its
//! *rendered label*, and graph-core's width table (`text_width.rs`) returns the `0.75` inch
//! default box for every id of up to three characters — the same box Graphviz gives those
//! labels (`docs/measurements/p13-gv2-dot.md`, Blocker 1). So the harness draws the **bare**
//! graph and both arms read the same width table; a pinned `box` column would be a second
//! width table, and the disagreement Blocker 1 names is one to *record*, not one to hide
//! behind a fixture.
//!
//! **Unlike the closed-form arms, this one records a disagreement** — see [`CEILING`].
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, so at a drawing
//! 293 inches across — the largest of these fixtures, seed 599 — one printed digit is 2.1e-1
//! points and no comparison can be tighter than that whatever the arithmetic. **That floor is
//! not what sets [`CEILING`] here**: the measured median is four orders of magnitude above it,
//! so the ceiling is a recorded disagreement and the floor is stated only so the gap is not
//! mistaken for a rounding one. The six closed cases are compared byte for byte in both arms,
//! where a tolerance is weaker than the truth.

use super::{Differential, coords};
use graph_core::layout::graphviz::dot;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const DOT: Differential = Differential {
    name: "dot",
    ceilings: &[("layout.dag.dot", "dot", CEILING)],
    line,
};

/// **1e+5 points**, the next power of ten above the worst gap measured over the 1000 gate
/// seeds. **This records a disagreement, not a printed resolution** — the difference from the
/// 1e-1 that `layout.twopi` and `layout.packing.osage` state, and the reason is measured.
///
/// | measurement | over the 1000 gate seeds |
/// |---|---|
/// | seeds at or under the printed resolution (1e-1) | **2** |
/// | median gap | **1.781e+03** points |
/// | worst gap | **1.851e+04** points, seed **587** |
/// | seeds carrying a gap above 1e+3 | **559** |
/// | seeds carrying a gap above 1e+4 | **142** |
///
/// **The cause is the order and position disagreements `docs/measurements/p13-gv2-dot.md`
/// already counts, not the oracle's formatter.** That file measures 692 of 1000 seeds
/// agreeing node for node on the rank, 408 of those agreeing on every rank's order, and only
/// **10** of the 408 printing every node centre exactly; the width table returns the `0.75`
/// inch default box for every id of up to three characters where the oracle is 3 to 16 points
/// wider, and that width is a *constraint length* in the x-coordinate simplex, so above ten
/// nodes the drawing differs by more than a shift. Only **2** seeds here sit at the printed
/// resolution against those 10, because this metric adds the uniform rescale over the whole
/// bounding box, which carries a rank or order disagreement out to every node.
///
/// The two seeds that do sit at it are 0 and 600 — the same two-node graph at two and four
/// character ids — and the next four are 1, 601, 2 and 602, which is the same five shapes
/// again. So the low tail is the fixture's small graphs, not the layout at large.
///
/// **No layout fix is attempted here.** The width table's escape hatch is one constant pair in
/// `text_width.rs`, outside this crate, and the 302 rank ties and 284 order disagreements are
/// properties of the optimum the two implementations reach. The row is therefore
/// `Status::Implemented`, never `gated`.
pub(crate) const CEILING: f64 = 1e5;

/// One seed's line: the gate's model, its bare graph structure for the harness to write DOT
/// from, and our own coordinates.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out["dot"] = coords(dot::ID, &nodes, &edges)?;
    Ok(out)
}
