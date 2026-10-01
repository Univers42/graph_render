//! Layouts whose oracle differential is not a byte comparison or has no recorded run,
//! so their row is `implemented`, never `gated`.

use super::super::Status;

/// Layouts held to a force-layout oracle rather than to a byte-exact one, keyed by the
/// record their differential writes.
///
/// Force layouts are the project's first layouts that **cannot** be byte-compared
/// against their oracle, and the reason is algorithmic rather than a shortfall: a
/// force simulation amplifies a 1-ULP difference into a different picture, so
/// "different, but no worse" is the strongest true claim available and identity is
/// not. Each therefore gets its own record, holding its own metric:
///
/// - `layout.force.barnes_hut` is held to **d3-force@3.0.0** (the frozen force set at
///   d3's own parameters) by the stress metric: Pearson hop/euclid correlation over 32
///   max-min pivots, margin **-0.05** against d3 on the same topology, the same golden
///   spiral seed positions and the same 112 ticks (`stress`; spec decision 6).
/// - `layout.forceatlas2` is held to **networkx 3.6 `forceatlas2_layout`** in the
///   `ge-python-oracle` image (`oracle-fa2`): a port of the whole function at its
///   defaults, compared with the pinned library rather than restated by hand.
/// - `layout.force.spring` is held to **networkx 3.6 `spring_layout` at `dim=2`** —
///   SciGraphs' own `SPRING` (`oracle-spring`), and by the same stress metric the other
///   force rows use, because a force simulation amplifies a 1-ULP difference into a
///   different picture exactly as it does for them.
/// - `layout.circular.hierarchy` is held to **SciGraphs' own
///   `_circular_hierarchy_layout`** in the same image with the submodule mounted
///   (`oracle-circular-hierarchy`). Unlike the three above this one *is* a closed form
///   and a coordinate gap is a fair comparison — but the record is still `implemented`,
///   and for the reason the clause below gives, not because the comparison is weak.
///
/// All four rows are `implemented`, not `gated`: `Status::Gated` is refused by
/// `problems()` unless *both* a 4-way hash verdict and the row's own oracle verdict
/// are backed by a recorded run on this tree, and `verdict::oracle_record` resolves only
/// the records the `Evidence` struct carries (`capabilities/verdict.rs:63-74`) — neither
/// `oracle-spring` nor `oracle-circular-hierarchy` is one of them, so a `gated` row here
/// could only ever read back "no oracle-spring record: run the gate" and report a refusal
/// where a verdict belongs. `implemented` states the truth: registered, hashed,
/// differentially measured, not yet an oracle-backed gate. Claiming `gated` for any of
/// them with only a hash behind it would be exactly the silent weakening of the project's
/// central guarantee the phase prompt forbids, and the fix is a `verdict::Evidence` arm
/// per differential — which belongs with the ledger change that would earn the status, not
/// smuggled in to make one row look stronger than the other three.
pub(super) fn force_record(id: &str) -> Option<(&'static str, Status)> {
    match id {
        "layout.force.barnes_hut" => Some(("stress", Status::Implemented)),
        "layout.forceatlas2" => Some(("oracle-fa2", Status::Implemented)),
        // Ponytail: no differential exists for the multilevel layout (not sfdp); the
        // stress record is the closest metric and is barnes_hut's, so `implemented` only.
        "layout.force.yifan_hu" => Some(("stress", Status::Implemented)),
        // Its own differential, and its own record, for the same reason `layout.forceatlas2`
        // gets one: the two FR ports share a metric but share no code, so one record
        // standing for both would let either be measured by the other's run.
        "layout.force.spring" => Some(("oracle-spring", Status::Implemented)),
        "layout.circular.hierarchy" => Some(("oracle-circular-hierarchy", Status::Implemented)),
        // Ponytail: `implemented`, not `gated`: the closed-form differential has no
        // recorded run on this tree, and a hash alone never earns `gated`.
        "layout.random" | "layout.circular.ring" | "layout.spiral" | "layout.bipartite" => {
            Some(("oracle-closed-form", Status::Implemented))
        }
        _ => None,
    }
}
