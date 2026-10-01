//! `layout.force.neato` against **Graphviz's own** `neato`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` the other differentials emit — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over, and the engine's output is reproducible by the `cmp` the ADR
//! records.
//!
//! **This arm's coordinates are another implementation's**, in points, and the ceiling is
//! [`CEILING`], the next power of ten above the worst gap measured over the 1000 gate seeds
//! (`docs/measurements/p13-gv2-neato.md`).
//!
//! Two things make this engine different from `twopi`, and both were measured rather than
//! assumed:
//!
//! - **`-Gstart` is load-bearing.** The reference seeds a `drand48` generator from it and
//!   reads two draws per node into its initial placement, so the drawing is a function of the
//!   seed — all 1000 fixtures draw differently at `-Gstart` 1, 7 and 99. This port reproduces
//!   the generator rather than a distribution, which is why `rng` is part of the port.
//! - **The engine is iterative, and the port reproduces its arithmetic rather than its
//!   intent.** The reference multiplies in `float` and accumulates in `double`, centres every
//!   vector before every solve, and stops on a relative change in the stress. The `f32`/`f64`
//!   split is load-bearing: widening the *product* as well as the accumulator, which reads
//!   like a harmless precision improvement, moved seed 44's gap from 4.7e-3 to 1.7e+02 points.
//!
//! The measured gap is nonetheless **at the oracle's printed resolution** (0.074 of one
//! printed digit at the largest drawing), so unlike an under-implemented port this one is not
//! limited by anything but the format. The sixth closed case is the exception and says so:
//! the 6-branch agrees to four significant digits, not five, which is the iterative part.
//!
//! Ponytail: `-Tplain` prints five significant digits, so at the largest gate drawing one
//! printed digit is 0.911 points. That is the floor on this comparison, and the measured gap
//! is a fraction of it. The six small cases are compared token for token in both arms, where
//! the drawing is small and the printed precision is relatively finer.

use super::{Differential, coords};
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

/// The neato arm of the Graphviz differential.
pub const NEATO: Differential = Differential {
    name: "neato",
    ceilings: &[("layout.force.neato", "neato", CEILING)],
    line,
};

/// The neato ceiling: the worst `max |ours - theirs|` in points over the 1000 gate seeds is
/// **6.73e-2**, rounded up to the next power of ten.
///
/// **The gap is the oracle's printed resolution, not a disagreement.** `-Tplain` writes five
/// significant digits, so at the largest gate drawing — 12.655 inches across, 911.2 points —
/// one printed digit is 0.911 points, and the worst gap is 0.074 of that. The evidence that
/// it *is* that quantum and nothing more: the per-seed gap tracks the drawing's extent with
/// correlation 0.71, the median is 1.61e-2, and **no seed exceeds 1e-1** (527 of 1000 are
/// above 1e-2, 401 above 3e-2, 0 above 1e-1). A genuine algorithmic difference would show up
/// as a handful of seeds at O(1) drawing widths, not as a distribution that moves with the
/// size of the drawing.
///
/// What is *not* at the resolution is the sixth closed case: the 6-branch agrees to the
/// fourth significant digit rather than the fifth, so one token of twelve differs. That is
/// the iterative part — 121 stress passes on six nodes — and it is why the ceiling is a
/// tolerance and not a claim of identity. The other five cases, including the 3-path's 132
/// passes, agree token for token. `docs/measurements/p13-gv2-neato.md` has the distribution.
///
/// A tighter ceiling would be a claim about `-Tplain`'s formatter rather than about the
/// layout, which is the same argument `docs/measurements/p13-gv1.md` makes for twopi.
pub(crate) const CEILING: f64 = 1e-1;

/// One seed's line: the gate's model, its bare graph structure for the harness to write
/// DOT from, and our own coordinates under each of this differential's keys.
///
/// The loop over [`NEATO`]'s own ceilings rather than a named column, so adding a second
/// Graphviz engine to this differential is a one-line change to the `ceilings` slice and
/// nothing here — the same shape `spectral` uses, and the reason the two differentials do
/// not drift apart. The emit's `--max-iter` (ForceAtlas2's knob) does not reach this
/// engine, which is iterative at the reference's own budget.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    for &(id, key, _) in NEATO.ceilings {
        out[key] = coords(id, &nodes, &edges)?;
    }
    Ok(out)
}
