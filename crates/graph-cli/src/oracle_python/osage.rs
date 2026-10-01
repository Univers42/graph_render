//! `layout.packing.osage` against **Graphviz's own** `osage`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py --differential`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` every other differential emits — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over, and the engine's output is reproducible by the `cmp` the ADR
//! records.
//!
//! **Every node carries an explicit box, and the DOT pins the same box.** Graphviz sizes a
//! node from its *rendered label* — 54 points for `n0`..`n9`, 57.942 for `n10`..`n99` — which
//! graph-core has no font engine for, and `arrayRects` sorts the boxes by `width + height`
//! with a `qsort` that is not stable, so among *tied* boxes the cell order is glibc's rather
//! than the graph's. Those were the two named causes of this differential disagreeing with
//! Graphviz by 122..1785 points on 982 of the 1000 seeds. Both are removed at the fixture
//! rather than absorbed by a ceiling, and the numbers are generated **once**:
//! [`osage::Boxes`] hands graph-core's layout one box per node and [`boxes_in_inches`] below
//! writes those same boxes into the fixture's `box` column, which
//! `harness/oracle-graphviz.py` turns into `fixedsize=true`, `width`, `height`, `label=""`
//! and `margin=0`. One table, one fixture, two arms reading the same numbers — and the
//! `negctl-osage-sizes` gate row proves the comparison notices when they stop agreeing.
//!
//! [`osage::Boxes::box_at`] rises strictly with the node index, so no two boxes tie on the
//! sort key and the descending sort `acmpf` performs is a **total order**: the tie is gone
//! instead of argued about, and the drawing is a different one from the uniform grid at every
//! node count rather than only past the eleventh. `-Gstart` is inert for this engine —
//! measured, not assumed.
//!
//! Ponytail: the oracle's `-Tplain` output carries five significant digits, so the gap is
//! bounded below by its formatter however exact the arithmetic is; [`CEILING`] states that
//! floor and the measurement behind it. The six analytically determined small cases are
//! compared byte for byte in both arms against the *default* `nodesize` — Graphviz's in the
//! harness, our own node by node in graph-core's tests — where a tolerance is weaker than
//! the truth. That arm is the ledger row's own path; this arm is the differential's.

use super::{Differential, points};
use graph_core::layout::graphviz::osage::{self, Boxes};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const OSAGE: Differential = Differential {
    name: "osage",
    ceilings: &[("layout.packing.osage", "osage", CEILING)],
    line,
};

/// The osage ceiling: **1e-1 points**, the next power of ten above the worst gap measured
/// over the 1000 gate seeds, which is **3.6e-2**.
///
/// **That worst gap is the oracle's own printed resolution, not a disagreement.** `-Tplain`
/// writes five significant digits (`lib/common/output.c:129-141`), so at the largest fixture
/// drawing — 20.8 inches across — one printed digit is 0.001 inch, which is **0.072 points**,
/// and the measured gaps run 0 to 3.6e-2, tracking the drawing's extent and never exceeding
/// half a printed digit. The ceiling is therefore a claim about `-Tplain`'s formatter, not
/// about the layout, which is what lets the row be `gated`: `docs/measurements/p13-gv1-osage.md`
/// records the per-seed distribution and the command that produced it.
///
/// The previous ceiling was 1e4 and recorded a **disagreement** rather than a resolution —
/// 122 to 1785 points on 982 of 1000 seeds, from the two causes the fixture removes. No
/// ceiling was widened to pass here; this one is 10^5 tighter than the gap it replaces, and
/// the same 1e-1 that `layout.twopi` states for the same printed-resolution reason.
pub(crate) const CEILING: f64 = 1e-1;

/// The fixture's `box` column: one `[width, height]` pair per node, **in inches**, because
/// the DOT's `width`/`height` attributes are inches and the harness writes them verbatim.
///
/// Points -> inches is `POINTS_PER_INCH` (`lib/common/geom.h:58`) in the direction Graphviz
/// goes, and on [`osage::Boxes`]' grid the round trip is exact — `points / 72 * 72` is the
/// same `f64` — so the box the harness pins and the box the layout packed are the same box
/// and not two that differ in the last bit. `graph-core`'s tests measure that.
fn boxes_in_inches(boxes: &Boxes) -> Value {
    let column: Vec<[f64; 2]> = boxes
        .get()
        .iter()
        .map(|node_box| {
            [
                node_box.w / osage::POINTS_PER_INCH,
                node_box.h / osage::POINTS_PER_INCH,
            ]
        })
        .collect();
    json!(column)
}

/// One seed's line: the gate's model, its bare graph structure for the harness to write DOT
/// from, the per-node boxes the harness pins, and our own coordinates over those same boxes.
///
/// The layout is closed form, so the emit's `--max-iter` (ForceAtlas2's) does not reach it.
/// This arm does **not** call the registry's `run`: it calls [`osage::run_sized`] over the
/// fixture table, which is the reference's general `arrayRects` rather than its uniform
/// special case. The ledger row still runs `run`, at Graphviz's default `nodesize`, and the
/// closed cases still pin that path byte for byte.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let boxes = Boxes::table(n);
    let geometry = osage::run_sized(&topology, &boxes).map_err(|e| e.to_string())?;
    Ok(json!({
        "seed": seed,
        "n": n,
        "source": columns.source,
        "target": columns.target,
        "box": boxes_in_inches(&boxes),
        "osage": points(osage::ID, &geometry.nodes)?,
    }))
}
