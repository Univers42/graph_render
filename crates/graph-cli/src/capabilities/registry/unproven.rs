//! Layouts whose oracle differential is not a byte comparison or has no recorded run,
//! so their row is `implemented`, never `gated`.

use super::super::Status;

/// The six igraph 2D layouts, held to `harness/oracle-igraph.py`'s stress ratio.
const IGRAPH_LAYOUTS: [&str; 6] = [
    "layout.force.fruchterman_reingold",
    "layout.force.kamada_kawai",
    "layout.force.drl",
    "layout.force.lgl",
    "layout.force.davidson_harel",
    "layout.force.graphopt",
];

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
///
/// Both rows are `implemented`, not `gated`: `Status::Gated` is refused by
/// `problems()` unless *both* a 4-way hash verdict and the row's own oracle verdict
/// are backed by a recorded run on this tree, and neither force differential has been
/// run to 1000 seeds here. Claiming `gated` for a force layout with only a hash behind
/// it would be exactly the silent weakening of the project's central guarantee the
/// phase prompt forbids.
pub(super) fn force_record(id: &str) -> Option<(&'static str, Status)> {
    match id {
        "layout.force.barnes_hut" => Some(("stress", Status::Implemented)),
        "layout.forceatlas2" => Some(("oracle-fa2", Status::Implemented)),
        // Ponytail: no differential exists for the multilevel layout (not sfdp); the
        // stress record is the closest metric and is barnes_hut's, so `implemented` only.
        "layout.force.yifan_hu" => Some(("stress", Status::Implemented)),
        id if IGRAPH_LAYOUTS.contains(&id) => Some(("oracle-igraph", Status::Implemented)),
        // Ponytail: `implemented`, not `gated`: the closed-form differential has no
        // recorded run on this tree, and a hash alone never earns `gated`.
        "layout.random" | "layout.circular.ring" | "layout.spiral" | "layout.bipartite" => {
            Some(("oracle-closed-form", Status::Implemented))
        }
        // Ponytail: `implemented`, not `gated`, and the reason is the oracle's own printed
        // resolution rather than a shortfall: `-Tplain` carries five significant digits, so
        // the twopi differential compares coordinates within a measured 7.1e-2 points (ceiling
        // 1e-1). It is routed to its own record so the row says which comparison backs it,
        // never `gated` on a hash alone.
        //
        // `oracle_diff` still reads `not backed: no oracle-twopi record` even after a real run,
        // because `verdict::Evidence::oracle_record` (`capabilities/verdict.rs:63-74`) matches
        // a fixed list of record names and has no arm for `oracle-twopi` — nor for
        // `oracle-closed-form`, which is why the four `implemented` rows above read the same
        // way. That is a pre-existing gap in the reader, not a claim this row is making: the
        // differential is real and its numbers are in `docs/measurements/p13-gv1.md`.
        "layout.twopi" => Some(("oracle-twopi", Status::Implemented)),
        // `layout.treemap.patchwork` is routed the same way and for the same reason, and
        // carries the same caveat as the twopi row above: `verdict::Evidence::oracle_record`
        // has no arm for `oracle-patchwork` either, so `oracle_diff` reads
        // `not backed: no oracle-patchwork record` even after the real run that wrote
        // `target/gates/oracle-patchwork.json` (1000 cases, worst 6.613e-2, pass). Same
        // pre-existing reader gap, not a claim this row is making: the differential is real
        // and its numbers are in `docs/measurements/p13-gv1-patchwork.md`, and the ceiling
        // reflects `-Tplain`'s five significant digits rather than a shortfall.
        "layout.treemap.patchwork" => Some(("oracle-patchwork", Status::Implemented)),
        _ => None,
    }
}
