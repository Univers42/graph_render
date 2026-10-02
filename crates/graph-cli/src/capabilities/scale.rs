//! Phase 9's three `scale` rows. Split out of `capabilities.rs` for the same reason
//! `analysis.rs` and `ingest.rs` are: the house's 300-line limit, and a fourth group of
//! rows would have pushed that file over it.
//!
//! `Status::Implemented`, never `Gated`: `graph-core`'s scale stage is not in the hash
//! gate's stage list and has no oracle differential, so `gated` would be a claim
//! `problems()` refuses for the right reason. Promoting them is the merge step's work,
//! recorded in `docs/reports/phase-09-progress.md`.
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
        oracle_record: "oracle-diff",
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
            oracle: "SciGraphs engine/scigraphs_engine/lod.py: the budget rule and the never-empty mask, ported; the pixel thresholds are not (no camera here) and the tier ladder is the phase's own",
            complexity: "O(n + m), one pass each, no spatial structure",
            degradation: "advisory by construction: the hints are columns a front may ignore entirely, and the topology is never mutated, so past the ceiling the only cost is a front that chose to draw everything",
            ponytail: "Ponytail: the thresholds are a heuristic and it fails in the dangerous direction. Failing input: a graph whose important nodes are low-degree (a dependency graph's entry points, a star's hub the budget ranks low), where a degree-ranked label budget hides exactly what a reader came for. Direction: hiding meaningful nodes. Escape hatch: ignore the hints; they are advisory. Second heuristic, same shape: edge decimation is a stride over edge index, so a graph whose long-range edges share one stride class loses all of them",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.simplify",
            oracle: "hand: degree-1 folding, maximal degree-2 chain walks, and Phase 7's analysis.communities (louvain) for the collapse. The reference's simplify.py extracts a backbone (MST/disparity/top-k) and coarsens for bundling; neither is a reversible reduction of the graph, so neither is ported",
            complexity: "O(n + m log m) to build the simple adjacency, then O(n + m) per pass",
            degradation: "past the ceiling, the same shape as topology: wasm32 cannot allocate and the module traps; natively, memory permitting, this refuses alongside index_model's own CapacityError. Every removal is journalled, so a front that ignored the ceiling would still be able to restore",
            ponytail: "Ponytail: the community collapse trusts louvain, a heuristic. Failing input: near-tied modularity gains, or a graph whose communities are single-edge chains, where a collapse removes the node a reader came to see. Direction: cosmetic, because the journal still holds it — the dangerous version, an irreversible collapse, is not implemented. Escape hatch: Plan::collapse_communities off",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.adaptive",
            oracle: "SciGraphs engine/scigraphs_engine/adaptive.py: the same intent (bounded work per settle), deliberately NOT its mechanism — it adapts from measured crowding and a camera at render time, which here would mean reading a clock, which D8 forbids inside the motor",
            complexity: "O(1): a pure function of (n, m)",
            degradation: "none: the budget never fails and never allocates. What degrades is the layout's settle at large n, which is the trade the row exists to make and the caller's iteration override takes back",
            ponytail: "Ponytail: a large graph gets fewer ticks and a less settled layout. Failing input: any graph past ~3 000 nodes, whose tails are still moving when the budget runs out. Direction: cosmetic. Escape hatch: tick_budget_with's explicit override, honoured verbatim including 0",
            ..common("", "", "", "")
        },
    ]
}
