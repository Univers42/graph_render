//! Phase 9's three `scale` rows. Split out of `capabilities.rs` for the same reason
//! `analysis.rs` and `ingest.rs` are: the house's 300-line limit, and a fourth group of
//! rows would have pushed that file over it.
//!
//! `Status::Implemented`, never `Gated`: `graph-core`'s scale stage is not in the hash gate's
//! stage list, so `gated` would be a claim `problems()` refuses for the right reason.
//! Promoting them is the merge step's work, recorded in `docs/reports/phase-09-progress.md`.
//!
//! `scale.lod` and `scale.simplify` are checked by the `oracle-scale` differential and name
//! its functions, so `verdict::oracle_diff` reads each one out of that record.
//! **`scale.adaptive` names no record at all**: `adaptive.py`'s cut needs a hierarchy of
//! coarse levels that `build_hierarchy` builds with an infomap detector, so it is a gap row
//! and a differential it cannot be compared by would only be a claim.
//!
//! `scale_ceiling` here is **inherited and reasoned, not measured**: these are `O(n)` and
//! `O(n + m)` passes over a topology that is already indexed, so they are bounded by
//! topology's own measured per-node cost, exactly as `analysis`'s rows are. The measured
//! ceiling campaign is [`super::CEILINGS_DOC`]'s subject.

use super::registry::TOPOLOGY_CEILING;
use super::{Capability, Status};

/// Phase 9's three `scale` rows, by the one name every family is reached under.
pub fn rows() -> Vec<Capability> {
    let ceiling = TOPOLOGY_CEILING;
    let common = |oracle: &'static str,
                  complexity: &'static str,
                  degradation: &'static str,
                  ponytail: &'static str| Capability {
        id: "",
        tier: 1,
        stage: "scale",
        geometry: None,
        status: Status::Implemented,
        oracle,
        oracle_record: "oracle-scale",
        // Every row below names its own, because the record's `functions` are keyed by the
        // ceiling ids of `oracle_python::SCALE` and a row may take one, two or none. A Rust
        // literal needs the field whatever the row says.
        functions: &[],
        hash_stage: "topology",
        oracle_diff: String::new(),
        hash_4way: String::new(),
        scale_ceiling: ceiling,
        degradation,
        ponytail,
        complexity,
    };
    vec![
        Capability {
            id: "scale.lod",
            oracle: "SciGraphs engine/scigraphs_engine/lod.py, checked by the oracle-scale differential: lod.apply_budget against lod.rs's label_mask (the greedy budget, its never-empty mask, budgets 0/1/n, and three whose cut lands inside a class of equal degrees, compared against the rule rather than np.argsort's order) and lod.frustum_cull_spheres against lod.rs's Viewport test, over a square orthographic camera. Not ported: the pixel thresholds (a headless motor has no pixels), the tier ladder (the phase's own), and the radius convention, which lod.py's two culling functions do not share between them — docs/measurements/fix-scale-oracle.md records that as a gap",
            functions: &["scale.lod.apply_budget", "scale.lod.frustum_cull_spheres"],
            complexity: "O(n + m), one pass each, no spatial structure",
            degradation: "advisory by construction: the hints are columns a front may ignore entirely, and the topology is never mutated, so past the ceiling the only cost is a front that chose to draw everything",
            ponytail: "Ponytail: the thresholds are a heuristic and it fails in the dangerous direction. Failing input: a graph whose important nodes are low-degree (a dependency graph's entry points, a star's hub the budget ranks low), where a degree-ranked label budget hides exactly what a reader came for. Direction: hiding meaningful nodes. Escape hatch: ignore the hints; they are advisory. Second heuristic, same shape: edge decimation is a stride over edge index, so a graph whose long-range edges share one stride class loses all of them",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.simplify",
            oracle: "hand: degree-1 folding, maximal degree-2 chain walks, and Phase 7's analysis.communities (louvain) for the collapse; reversibility is graph-core's own gate row. What the oracle-scale differential checks against SciGraphs simplify.build_coarse_level is the collapse's link set — the external edges re-anchored on the representatives — and that a self-loop, and an edge between two members of one community, are in neither. Not ported: the backbone (MST/disparity/top-k), which like the coarse level is not a reversible reduction of the graph. The reference's only self-loop rule is that one, and it covers the coarse level only; leaf folding and chain contraction have no reference function at all and are gap rows",
            functions: &["scale.simplify.build_coarse_level"],
            complexity: "O(n + m log m) to build the simple adjacency, then O(n + m) per pass",
            degradation: "past the ceiling, the same shape as topology: wasm32 cannot allocate and the module traps; natively, memory permitting, this refuses alongside index_model's own CapacityError. Every removal is journalled, so a front that ignored the ceiling would still be able to restore",
            ponytail: "Ponytail: the community collapse trusts louvain, a heuristic. Failing input: near-tied modularity gains, or a graph whose communities are single-edge chains, where a collapse removes the node a reader came to see. Direction: cosmetic, because the journal still holds it — the dangerous version, an irreversible collapse, is not implemented. Escape hatch: Plan::collapse_communities off",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.adaptive",
            oracle: "SciGraphs engine/scigraphs_engine/adaptive.py: the same intent (bounded work per settle), deliberately NOT its mechanism — it adapts from measured crowding and a camera at render time, which here would mean reading a clock, which D8 forbids inside the motor. A gap row in the oracle-scale differential: adaptive.py cuts a hierarchy of coarse levels that build_hierarchy builds with an infomap detector, and the motor has neither, so the two arms have nothing they could be given in common. Hence no oracle_record: there is no reference function this row could be compared against",
            oracle_record: "",
            functions: &[],
            complexity: "O(1): a pure function of (n, m)",
            degradation: "none: the budget never fails and never allocates. What degrades is the layout's settle at large n, which is the trade the row exists to make and the caller's iteration override takes back",
            ponytail: "Ponytail: a large graph gets fewer ticks and a less settled layout. Failing input: any graph past ~3 000 nodes, whose tails are still moving when the budget runs out. Direction: cosmetic. Escape hatch: tick_budget_with's explicit override, honoured verbatim including 0",
            ..common("", "", "", "")
        },
    ]
}
