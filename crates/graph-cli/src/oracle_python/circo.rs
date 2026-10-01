//! `layout.circular.circo` against **Graphviz's own** `circo`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py` in its differential mode).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` every other differential emits — so the `n`,
//! `source` and `target` the harness writes DOT from are the fixtures the engine is run over.
//!
//! **`-Gstart` is inert for this engine, measured rather than assumed**: over a strided 20-seed
//! subset spanning `n = 2..552` the output is byte-identical at `start=1`, `start=7` and
//! `start=99`, and identical across two runs at the same seed. So the gap below is an
//! algorithmic difference or nothing, never seed drift.
//!
//! **The gap is a slot order, not a radius.** Over the 1000 gate seeds the worst gap is
//! 6.460e+04 points (seed 553), while all 14 analytically determined closed cases agree byte
//! for byte. The difference is which node occupies which slot on a block's circle:
//! `remove_pair_edges` orders its degree list with `LIST_SORT`, which is `qsort`
//! (`lib/util/list.c:363`), and glibc 2.41 does not make that stable, so a block with two
//! nodes of equal degree can be thinned in either order. This port sorts stably and lands on
//! the other, equally valid, circle order. The named cause and the way out are in
//! `docs/measurements/p13-gv1-circo.md`; see [`CEILING`].

use super::{Differential, coords};
use graph_core::layout::graphviz::circo;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const CIRCO: Differential = Differential {
    name: "circo",
    ceilings: &[("layout.circular.circo", "circo", CEILING)],
    line,
};

/// The circo ceiling: the worst `max |ours - theirs|` in points over the 1000 gate seeds is
/// **6.460e+04** (seed 553, `n = 555`), rounded up to the next power of ten.
///
/// That is a large number and it is the honest one. It is **not** a radius or a scale error:
/// on every one of the 1000 seeds the block decomposition, the block's radius
/// `N * (min_dist + largest_node) / 2*PI` and the closed cases agree with Graphviz, and the
/// drawing differs only in *which node sits in which slot* — the circle order that
/// `remove_pair_edges`'s degree sort and `reduce_edge_crossings` decide. The reference sorts
/// that list with `qsort`, which glibc 2.41 does not make stable, so the tie order is not
/// reproducible from the algorithm and this port's stable sort picks a different (equally
/// valid) order on a block with two nodes of equal degree. A different order on the same
/// circle is a different drawing, not a wrong one, and the gap is the size of the circle.
///
/// `docs/measurements/p13-gv1-circo.md` carries the per-`n` table, the named cause and the
/// way out. The ceiling is never widened to make a row pass, and the row is
/// `Status::Implemented`, never `gated`: a hash over our own bytes cannot say we match
/// Graphviz, and the measurement above says we do not.
pub(crate) const CEILING: f64 = 1e5;

/// One seed's line: the gate's model, its bare graph structure for the harness to write DOT
/// from, and our own coordinates. The layout is closed form in the sense that it has no
/// iteration budget, so the emit's `--max-iter` (ForceAtlas2's) does not reach it.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out["circo"] = coords(circo::ID, &nodes, &edges)?;
    Ok(out)
}
