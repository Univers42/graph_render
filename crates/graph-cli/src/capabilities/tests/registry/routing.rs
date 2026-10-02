//! Which record each ledger row's two verdicts are read from, and that every oracle
//! function is covered exactly once.
//!
//! **Split out of the parent for the house line cap.** This is the routing test: it holds the
//! `id -> (oracle_record, hash_stage)` mapping for every capability, so it is the one file a
//! reader opens to answer "what is this row measured against". `layout.basic3d.spiral` is
//! the newest entry in it and the reason it moved: it is routed to `scigraphs-conformance`,
// NOT to `oracle-basic-3d`, whose `ARMS` covers only sphere, helix and cube.

use super::super::*;
use super::ids::{
    BASIC_3D as BASIC_3D_IDS, BASIC_3D_UNDIFFERENTIALLED as BASIC_3D_UNDIFFERENTIALLED_IDS,
    IGRAPH as IGRAPH_LAYOUT_IDS,
};
use super::records_of;
use std::collections::BTreeSet;

#[test]
fn the_registry_covers_every_oracle_function_once_its_ids_are_unique() {
    let rows = registry();
    let mut covered: Vec<&str> = rows
        .iter()
        .filter(|r| r.oracle_record == "oracle-diff")
        .flat_map(|r| r.functions.iter().copied())
        .collect();
    covered.sort_unstable();
    covered.dedup();
    let mut want = COVERED.to_vec();
    want.sort_unstable();
    assert_eq!(covered, want);
    let ids: BTreeSet<&str> = rows.iter().map(|r| r.id).collect();
    assert_eq!(ids.len(), rows.len());
    for r in &rows {
        let expected = if r.id.starts_with("topology.") {
            ("oracle-diff", "topology", Status::Gated)
        } else if r.id.starts_with("analysis.") {
            ("oracle-diff", "analysis", Status::Implemented)
        } else if r.id.starts_with("ingest.") || r.id.starts_with("adapter.") {
            // Phase 10: honest, not yet evidence-backed. The oracles these rows have are
            // the convergence fixture and the contract round trip, neither of which is a
            // recorded gate run yet, so `Implemented` is what the evidence supports.
            ("roundtrip", "ingest.build", Status::Implemented)
        } else if r.id == "layout.tree.tidy" || r.id == "layout.treemap.squarified" {
            ("oracle-layouts", r.id, Status::Gated)
        } else if r.id == "layout.force.barnes_hut"
            || r.id == "layout.force.yifan_hu"
            || r.id == "layout.forceatlas2.barnes_hut"
        {
            ("stress", r.id, Status::Implemented)
        } else if r.id == "layout.forceatlas2" {
            ("oracle-fa2", r.id, Status::Implemented)
        } else if IGRAPH_LAYOUT_IDS.contains(&r.id) {
            ("oracle-igraph", r.id, Status::Implemented)
        } else if r.id == "layout.force.spring" || r.id == "layout.force.spring3d" {
            // **One record for both, deliberately**: one algorithm at two dimensions over one
            // kernel, so one stress run at `dim = 3` is spring3d's comparison.
            ("oracle-spring", r.id, Status::Implemented)
        } else if BASIC_3D_IDS.contains(&r.id) {
            // One arm file, one record — `implemented` per `unproven.rs`.
            ("oracle-basic-3d", r.id, Status::Implemented)
        } else if BASIC_3D_UNDIFFERENTIALLED_IDS.contains(&r.id) {
            // **Not `oracle-basic-3d`, and the separation is the point**: that arm's `ARMS`
            // covers three functions and not this one, so naming it here would claim a
            // differential nobody runs. Held to the conformance gate's own byte comparison
            // against SciGraphs until `sg-basic3d-spiral-oracle` adds `--function spiral`.
            ("scigraphs-conformance", r.id, Status::Implemented)
        } else if r.id == "layout.hierarchical3d" {
            ("oracle-hierarchical-3d", r.id, Status::Implemented)
        } else if r.id == "layout.bipartite_3d" {
            // The conformance gate's own record, and not `oracle-closed-form`: that is
            // `layout.bipartite`'s, over networkx's two columns.
            ("scigraphs-conformance", r.id, Status::Implemented)
        } else if r.id == "layout.circular.hierarchy" {
            // A closed form with a SciGraphs-arm differential, `implemented` rather than
            // `gated` for the reason `unproven.rs` gives: the record is read by name like
            // any other, and what is missing is a 4-way negative control on this stage.
            ("oracle-circular-hierarchy", r.id, Status::Implemented)
        } else if [
            "layout.random",
            "layout.circular.ring",
            "layout.spiral",
            "layout.bipartite",
        ]
        .contains(&r.id)
        {
            ("oracle-closed-form", r.id, Status::Implemented)
        } else if r.id == "layout.twopi" {
            // The Graphviz arm: its own record, and `implemented` rather than `gated`
            // because the differential compares coordinates within a measured 7.1e-2 points
            // (`docs/measurements/p13-gv1.md`) rather than to bytes.
            ("oracle-twopi", r.id, Status::Implemented)
        } else if r.id == "layout.force.neato" {
            // Also a Graphviz arm, and also `implemented` for the same reason as the row
            // above it: the measured worst gap is 6.73e-2 points against a ceiling of 1e-1,
            // and it is the oracle's printed resolution rather than a disagreement
            // (`docs/measurements/p13-gv2-neato.md`).
            ("oracle-neato", r.id, Status::Implemented)
        } else if r.id == "layout.packing.osage" {
            // The Graphviz arm whose differential is measured and passing — worst gap
            // 6.309e-2 points under a 1e-1 ceiling over 1000 seeds
            // (`docs/measurements/p13-gv1-osage.md`) — and `gated`, because
            // `GM_MUTATE_PACKING_OSAGE_NODES` is the negative control behind the
            // `layout.packing.osage` stage. That control was the whole of what stood between
            // this row and `gated`, and `capabilities::tests::graphviz` tests it both ways.
            ("oracle-osage", r.id, Status::Gated)
        } else if r.id == "layout.circular.circo" {
            // The third Graphviz arm, on the `layout.twopi` reasoning and with a measured
            // disagreement to show for it: the sweep runs and it disagrees with the oracle
            // by 6.460e+04 points on 984 of the 1000 seeds, for one named cause outside the
            // motor (`docs/measurements/p13-gv1-circo.md`). `implemented`, never `gated`.
            ("oracle-circo", r.id, Status::Implemented)
        } else if r.id == "layout.treemap.patchwork" {
            // The Graphviz arm, same shape as twopi's and for the same reason: its
            // differential compares coordinates within a measured 6.6e-2 points
            // (`docs/measurements/p13-gv1-patchwork.md`) rather than to bytes, so the row is
            // `implemented` and never a `gated` claim resting on a hash.
            ("oracle-patchwork", r.id, Status::Implemented)
        } else if r.id == "layout.force.fdp" {
            // The third Graphviz arm, and the one that cannot be compared to bytes at all:
            // the pinned Graphviz 16.1.0 `fdp -Tplain -Gstart=1` disagrees with *itself*
            // over the same sweep, so no ceiling measured against it bounds anything and
            // `gated` would be a claim the oracle itself contradicts
            // (`docs/measurements/p13-gv2-fdp.md`). Its own record, like twopi's and
            // osage's — the three arms share an engine family and share no code.
            ("oracle-fdp", r.id, Status::Implemented)
        } else if r.id == "layout.force.sfdp" {
            // The fourth Graphviz arm, and the one whose `implemented` status has the
            // strongest reason of the four: this engine is seed-sensitive, and the oracle
            // compared *against itself* at `-Gstart` 7 rather than 1 already differs by up to
            // 4.81e+2 points on the differential's own metric — larger than the 3.88e+2 gap
            // our own arm shows
            // (`docs/measurements/p13-gv2-sfdp.md`). The measured gap between the two arms is
            // 3.881e+2, the same phenomenon — the reference randomises its multilevel
            // matchings and this port does not. `implemented` is the honest status; `gated`
            // would claim an agreement the job did not reach.
            ("oracle-sfdp", r.id, Status::Implemented)
        } else if r.id == "layout.spectral" || r.id == "layout.mds.pivot" {
            ("oracle-spectral", r.id, Status::Gated)
        } else if r.id == "transport.wasm.columnar" {
            ("wasm-transport", r.id, Status::Gated)
        } else if r.id == "sdk.js" {
            ("sdk-smoke", r.id, Status::Implemented)
        } else if r.id.starts_with("post.") {
            // Every POST row's hash stage is its own id: no POST stage is in the gate's
            // stage list yet, so a shared `post` name would claim a stage nothing hashes.
            ("roundtrip", r.id, Status::Implemented)
        } else if r.stage == "scale" {
            // Phase 9: not in the hash gate's stage list and no oracle differential, so
            // `implemented` until the merge step wires them.
            ("oracle-diff", "topology", Status::Implemented)
        } else {
            ("roundtrip", r.id, Status::Gated)
        };
        assert_eq!(
            (r.oracle_record, r.hash_stage, r.status),
            expected,
            "{}",
            r.id
        );
    }
    let sdk = rows.iter().find(|r| r.id == "sdk.js").expect("row");
    assert_eq!(records_of(sdk), ("sdk-smoke", "sdk.js"));
}
