//! `layout.force.fdp` against **Graphviz's own** `fdp`, in the docker-only
//! `ge-graphviz-oracle` image (`harness/oracle-graphviz.py --differential`).
//!
//! The fixture graph is the gate's own model — `gate_node_count(seed)` nodes at
//! `REFERENCE_DEGREE`, the same `seeded_model` every other differential emits — so the
//! emitted `n`, `source` and `target` are the graph `harness/oracle-graphviz.py` already
//! runs the engine over.
//!
//! **This arm's coordinates are another implementation's, and that implementation is not
//! reproducible.** Unlike `twopi` and `osage`, where two runs of the oracle are
//! byte-identical, `fdp -Tplain -Gstart=1` gives byte-different output on two runs over the
//! same graph: it is byte-stable for `-Gmaxiter` up to 99 and is not at the default 600, and
//! the divergence survives `-Goverlap=0`, so it is in the expansion phase rather than in the
//! packing. `initPositions` seeds the whole initial placement from `-Gstart`
//! (`tlayout.c:487`), and 300 cooled ticks of a Fruchterman-Reingold model amplify a
//! last-digit difference into a different drawing. The full bisection, the `MALLOC_PERTURB_`
//! control that rules out uninitialised memory, and the measured oracle **self-gap** over
//! the same 1000 seeds are in `docs/measurements/p13-gv2-fdp.md`; the self-gap is the floor
//! on any agreement this differential can report.
//!
//! Ponytail: the six closed shapes *are* stable over four oracle runs each, so they are
//! compared byte for byte in both arms, and the seeded placement is compared exactly — the
//! oracle run at `-Gmaxiter=1 -Goverlap=true` prints the placement and nothing else. What is
//! not comparable is the force result at those sizes, and the row stays
//! `Status::Implemented` for that reason rather than on a hash.

use super::{Differential, coords};
use graph_core::layout::graphviz::fdp;
use graph_core::{REFERENCE_DEGREE, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const FDP: Differential = Differential {
    name: "fdp",
    ceilings: &[("layout.force.fdp", "fdp", CEILING)],
    line,
};

/// The fdp ceiling: **1e6 points**, the next power of ten above the worst gap measured over
/// the gate seeds, which is 4.5e5 points at seed 466 (n = 468).
///
/// **This ceiling records how far apart two runs of the same algorithm are, not how
/// inaccurate this port is.** Three measurements, all in `docs/measurements/p13-gv2-fdp.md`:
///
/// 1. The two arms agree to under a point on the **two-node** seed only — 2.5e-5 points — and
///    at n = 3 they are already 14 points apart. There is no regime in which the gap is a small
///    number times a large n; the comparison simply stops being informative past n = 2.
/// 2. The oracle **disagrees with itself** over the same 1000 seeds: two full positional runs
///    are byte-identical on 961 and differ on 39, by up to 2.16e3 points. A 300-tick cooled
///    force model amplifies a last-digit difference into a different picture, and whether a
///    given graph is stable is a property of that graph — seed 620 (n = 22) gives 8 distinct
///    outputs in 8 runs while seed 763 (n = 165) is byte-stable.
/// 3. The metric is absolute and these drawings are not small — the largest in the sweep is
///    448 358 points across — so 4.5e5 points is a few tenths of a percent of the canvas.
///
/// So the gap above is dominated by measurements 2 and 3 and cannot be tightened while the
/// pinned Graphviz 16.1.0 `fdp` is not reproducible on its own inputs.
///
/// What the port *does* reproduce is the part that is not iterative: run the oracle at
/// `-Gmaxiter=1 -Goverlap=true` and `-Tplain` prints the seeded initial placement and nothing
/// else, and this port matches it to the oracle's printed quantum on the two-node and
/// three-node cases — which pins `K`, the box formula, the seed and the whole `drand48`
/// sequence at once.
///
/// The row stays `Status::Implemented` and is never `gated`, because a ceiling the reference
/// cannot meet against itself cannot be a bound on anything. What would make one meaningful is
/// a reproducible oracle, or an oracle arm that runs the engine twice and gates on the pair's
/// own spread.
pub(crate) const CEILING: f64 = 1e6;

/// One seed's line: the gate's model, its bare graph structure for the harness to write DOT
/// from, and our own coordinates. The engine's iteration budget is its own `maxiter`
/// attribute, not the emit's `--max-iter`, so that argument does not reach it.
fn line(seed: u32, _max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    out["fdp"] = coords(fdp::ID, &nodes, &edges)?;
    Ok(out)
}
