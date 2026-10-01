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
/// - `layout.force.spring` is held to **networkx 3.6 `spring_layout` at `dim=2`** —
///   SciGraphs' own `SPRING` (`oracle-spring`), and by the same stress metric the other
///   force rows use, because a force simulation amplifies a 1-ULP difference into a
///   different picture exactly as it does for them.
/// - `layout.circular.hierarchy` is held to **SciGraphs' own
///   `_circular_hierarchy_layout`** in the same image with the submodule mounted
///   (`oracle-circular-hierarchy`). Unlike the three above this one *is* a closed form
///   and a coordinate gap is a fair comparison — but the record is still `implemented`,
///   and for the reason the clause below gives, not because the comparison is weak.
/// - the six igraph-family layouts in [`IGRAPH_LAYOUTS`] are held to
///   **harness/oracle-igraph.py** (`oracle-igraph`), one row per layout.
/// - the five 3D layouts p12-t3 added — `layout.basic3d.sphere`, `.helix` and `.cube`
///   (three closed forms over `(num_nodes, scale)` sharing one arm,
///   `harness/oracle-basic-3d.py`), `layout.hierarchical3d` (the SciGraphs function
///   itself, `harness/oracle-hierarchical-3d.py`) and `layout.force.spring3d` — are held
///   as named in their own arms below. The first four are closed forms compared within a
///   coordinate tolerance, exactly like `oracle-closed-form`; `spring3d` shares
///   `layout.force.spring`'s record because it is that layout at `dim = 3`.
///
/// These rows are `implemented`, not `gated`, and none of them is `gated` for want of a
/// record the ledger can read: `verdict::Evidence` now resolves every record in the gates
/// directory by the name its own file carries, so `oracle-spring`,
/// `oracle-circular-hierarchy` and `oracle-igraph` read exactly as the Graphviz six do and a
/// new engine's differential needs no edit in the ledger's reader. `Status::Gated` is
/// refused by `problems()` unless *both* a 4-way hash verdict and the row's own oracle
/// verdict are backed by a recorded run on this tree, and for these rows one of those two is
/// not available: either the oracle is not byte-comparable at all (`oracle-igraph`,
/// `oracle-spring`), or the stage has no negative control behind it. `implemented` states
/// the truth: registered, hashed, differentially measured, not yet an oracle-backed gate.
/// Claiming `gated` for any of them with only a hash behind it would be exactly the silent
/// weakening of the project's central guarantee the phase prompt forbids.
pub(super) fn force_record(id: &str) -> Option<(&'static str, Status)> {
    match id {
        "layout.force.barnes_hut" => Some(("stress", Status::Implemented)),
        "layout.force.particle_mesh" => Some(("stress-pm", Status::Implemented)),
        "layout.forceatlas2" => Some(("oracle-fa2", Status::Implemented)),
        // Ponytail: no differential exists for SciGraphs' own multilevel layout; the
        // stress record is the closest metric and is barnes_hut's, so `implemented` only.
        // (Graphviz's `sfdp` is a different algorithm and has its own differential below.)
        "layout.force.yifan_hu" => Some(("stress", Status::Implemented)),
        // Its own differential, and its own record, because this engine is not
        // reproducible: the pinned Graphviz 16.1.0 `fdp -Tplain -Gstart=1` gives
        // byte-different output on two runs over the same graph, so no ceiling measured
        // against it can be a bound on anything, and `gated` on a hash alone would be a
        // claim the oracle itself contradicts. `implemented` is the honest status; the
        // measured oracle self-gap is the floor on agreement and is written up in
        // docs/measurements/p13-gv2-fdp.md.
        "layout.force.fdp" => Some(("oracle-fdp", Status::Implemented)),
        id if IGRAPH_LAYOUTS.contains(&id) => Some(("oracle-igraph", Status::Implemented)),
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
        // Ponytail: `implemented`, not `gated`, and the reason is the oracle's own printed
        // resolution rather than a shortfall: `-Tplain` carries five significant digits, so
        // the twopi differential compares coordinates within a measured 7.1e-2 points (ceiling
        // 1e-1). It is routed to its own record so the row says which comparison backs it,
        // never `gated` on a hash alone.
        //
        // `oracle_diff` reads that record — the ledger resolves it by name like any other —
        // and says "within measured ceiling of the oracle/1000 seeds" after a real run. The
        // numbers are in `docs/measurements/p13-gv1.md`.
        "layout.twopi" => Some(("oracle-twopi", Status::Implemented)),
        // Ponytail: `implemented`, not `gated`, and the reason is the oracle's own printed
        // resolution rather than a shortfall: `-Tplain` carries five significant digits, so
        // the neato differential compares coordinates within a measured 6.73e-2 points
        // (ceiling 1e-1) at a drawing where one printed digit is 0.911 points. The worst
        // gap is 0.074 of that quantum and no seed exceeds it, so this is the same shape of
        // claim the twopi row above makes — and the same reason it is not `gated`: a hash
        // alone never earns that, and an iterative engine's sixth closed case (the 6-branch)
        // agrees to four significant digits rather than five, so identity is not available
        // to claim. `docs/measurements/p13-gv2-neato.md` has the distribution.
        //
        // The record is `oracle-neato`, the name `oracle_python::graphviz::NEATO.name` gives it.
        "layout.force.neato" => Some(("oracle-neato", Status::Implemented)),
        // Ponytail: `implemented`, not `gated`, and the reason is on the **hash** side, not
        // the oracle side: this row's `oracle-osage` record is read and passes at a measured
        // worst gap of 6.309e-2 points under a 1e-1 ceiling
        // (docs/measurements/p13-gv1-osage.md), but `Status::Gated` also needs a negative
        // control that went red on the `layout.packing.osage` stage, and none does: the
        // honest run hashes the stage 4-way on every seed, while the controls that do go
        // red diverge `topology`, `layout.treemap.squarified` and `layout.packing.circle`
        // and leave this stage equal. The per-stage knob for a Graphviz engine is
        // deliberately absent from `hashgate::knobs` because that table is shared with the
        // parallel engine jobs (scripts/orch/rows/p13-gv1-osage.rows:23-31), so promoting
        // this row to `gated` needs that knob and a `negctl-osage-nodes` row — a change in
        // `hashgate/`, not in the ledger. `capabilities::tests::graphviz` names the gap and
        // shows a red control on this stage is the whole of what is missing.
        "layout.packing.osage" => Some(("oracle-osage", Status::Implemented)),
        // Ponytail: the same honest status and the same reason as `layout.twopi` above, for
        // the same Graphviz oracle, and a stronger reason than `layout.packing.osage` has:
        // this differential was *run* over the 1000 gate seeds and it disagrees with
        // Graphviz by 6.460e+04 points on 984 of them, for one named cause outside the
        // motor — the tie order in `remove_pair_edges`'s degree sort is `qsort`'s, and
        // glibc 2.41 does not make that stable (`docs/measurements/p13-gv1-circo.md`). The
        // blocks, the radii and the 14 closed cases all agree, so the drawings differ only
        // in which node takes which slot, and an agreement that narrow earns `implemented`
        // and nothing more.
        //
        // Its `oracle-circo` record is read like any other, and it records the disagreement
        // above rather than hiding it: 1000 cases, worst 6.460e+04, which is why the row is
        // not `gated` on it.
        "layout.circular.circo" => Some(("oracle-circo", Status::Implemented)),
        // `layout.treemap.patchwork` is routed the same way and for the same reason as
        // `layout.twopi`: its own record (`target/gates/oracle-patchwork.json`, 1000 cases,
        // worst 6.613e-2, pass) is read by name and the ceiling reflects `-Tplain`'s five
        // significant digits rather than a shortfall. Numbers in
        // `docs/measurements/p13-gv1-patchwork.md`.
        "layout.treemap.patchwork" => Some(("oracle-patchwork", Status::Implemented)),
        // ---- p12-t3: the five 3D layouts. Three closed forms over `(num_nodes, scale)`
        // that read no graph at all, so ONE arm file covers all three and each gets its
        // own record only because each is a different function with a different oracle
        // (`--function sphere|helix|cube` in one arm file, `harness/oracle-basic-3d.py`).
        // The reason they are `implemented` and not `gated` is the one the clause above
        // gives: `verdict::Evidence::oracle_record` has no arm for `oracle-basic-3d`, so a
        // `gated` row could only read back "run the gate" where a verdict belongs.
        "layout.basic3d.sphere" | "layout.basic3d.helix" | "layout.basic3d.cube" => {
            Some(("oracle-basic-3d", Status::Implemented))
        }
        // Its own record, not `oracle-closed-form`'s: it is the SciGraphs function itself
        // being compared, and `oracle-closed-form` is the networkx arm. Same `implemented`
        // reason as the two above.
        "layout.hierarchical3d" => Some(("oracle-hierarchical-3d", Status::Implemented)),
        // **The same record as `layout.force.spring`, deliberately.** They are one
        // algorithm at two dimensions over one kernel (`spring3d.rs` is `spring.rs` with
        // `D = 3`), so one stress-ratio measurement run at `dim = 3` is the comparison
        // this row names — and a second record would let either dimension be "measured"
        // by the other's run. What differs is not the record but the note: this row's
        // differential is `oracle-spring` re-run at `dim = 3`, and its gate is the stress
        // deficit at that dimension, not at two.
        "layout.force.spring3d" => Some(("oracle-spring", Status::Implemented)),
        // `layout.force.sfdp` is routed the same way, and its `Ponytail` caveat is stronger
        // than the three rows above rather than weaker, so it is worth stating why the row is
        // `Implemented` and not `Gated` on the hash alone. The differential is real and its
        // numbers are in `docs/measurements/p13-gv2-sfdp.md`. But this engine is SEED-SENSITIVE
        // (measured: the same fixture hashes differently at `-Gstart` 1, 7 and 99), and the
        // decisive number is that the oracle compared **against itself** at `-Gstart` 7 rather
        // than 1 differs by up to 4.81e+2 points on the differential's own metric over the
        // same 1000 seeds — LARGER than the 3.88e+2 gap our own arm shows. So the
        // measured gap between the two arms is not a shortfall this port can close by writing
        // better code: the reference draws a random permutation to order its multilevel
        // matchings, and a port that does not draw glibc's exact permutation stream cannot
        // land far below the oracle's own seed-to-seed spread. `Implemented` is the honest
        // status; `Gated` would claim a byte-agreement this job did not reach, and widening
        // the ceiling until the row passed would be the same claim with a bigger number.
        "layout.force.sfdp" => Some(("oracle-sfdp", Status::Implemented)),
        _ => None,
    }
}
