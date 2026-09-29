//! The registry's own shape: coverage, unique ids, the record each row's two verdicts are
//! read from, and the ledger row's serialised form (`prompt.md` §8). Split out of the
//! parent test module to keep both files under the house line limit.

use super::*;
use std::collections::BTreeSet;

/// The record a row names, as `(oracle_record, hash_stage)`. Each row's two names must be
/// a record `graph-cli` actually writes — a name nothing writes is a row that can never
/// be backed, however often the gate is re-run.
fn records_of(row: &Capability) -> (&'static str, &'static str) {
    (row.oracle_record, row.hash_stage)
}

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
<<<<<<< HEAD
    for r in &rows {
        let expected = if r.id.starts_with("topology.") {
            ("oracle-diff", "topology", Status::Gated)
        } else if r.id.starts_with("analysis.") {
            ("oracle-diff", "analysis", Status::Implemented)
        } else if r.id == "layout.tree.tidy" || r.id == "layout.treemap.squarified" {
            ("oracle-layouts", r.id, Status::Gated)
        } else if r.id == "layout.force.barnes_hut" {
            ("stress", r.id, Status::Implemented)
        } else if r.id == "layout.forceatlas2" {
            ("oracle-fa2", r.id, Status::Implemented)
        } else if r.id == "layout.spectral" || r.id == "layout.mds.pivot" {
            ("oracle-spectral", r.id, Status::Gated)
=======
}

#[test]
fn every_row_but_the_sdks_names_a_record_the_gates_really_write() {
    let rows = registry();
    // `transport.wasm.columnar` is `Gated`: the hash gate hashes it as a stage of its own
    // and records the per-seed tally that says the real ABI reached the shim's bytes, so
    // both of its verdicts come from a real record. `sdk.js` is `Implemented`: its gate is
    // a smoke script, not a recorded seed sweep, and a `gated` claim would be one
    // `capabilities --check` must refuse. Every topology and layout row stands `Gated`.
    let status = |id: &str| rows.iter().find(|r| r.id == id).expect("row").status;
    assert_eq!(status("transport.wasm.columnar"), Status::Gated);
    assert_eq!(status("sdk.js"), Status::Implemented);
    let gated: Vec<&Capability> = rows.iter().filter(|r| r.id != "sdk.js").collect();
    assert!(
        gated.iter().all(|r| r.status == Status::Gated),
        "{:?}",
        gated.iter().map(|r| (&r.id, r.status)).collect::<Vec<_>>()
    );
    for r in &gated {
        let expected = if r.id.starts_with("topology.") {
            ("oracle-diff", "topology")
        } else if r.id == "transport.wasm.columnar" {
            ("wasm-transport", "transport.wasm.columnar")
>>>>>>> origin/p4
        } else {
            ("roundtrip", r.id, Status::Gated)
        };
<<<<<<< HEAD
        assert_eq!(
            (r.oracle_record, r.hash_stage, r.status),
            expected,
            "{}",
            r.id
        );
=======
        assert_eq!(records_of(r), expected, "{}", r.id);
>>>>>>> origin/p4
    }
    let sdk = rows.iter().find(|r| r.id == "sdk.js").expect("row");
    assert_eq!(records_of(sdk), ("sdk-smoke", "sdk.js"));
}

/// The two force rows are `implemented`, and the *reason* is structural rather than
/// provisional: `Status::Gated` means a 4-way hash **and** an oracle differential
/// passed on this tree, and a force layout's differential is a margin rather than a
/// byte-equality (a force simulation amplifies a 1-ULP difference into a different
/// picture). Each names its own record, so neither can be promoted by borrowing the
/// other's evidence.
#[test]
fn a_force_row_is_implemented_and_names_its_own_oracle_record() {
    let rows = registry();
    for (id, record) in [
        ("layout.force.barnes_hut", "stress"),
        ("layout.forceatlas2", "oracle-fa2"),
    ] {
        let row = rows.iter().find(|r| r.id == id).expect("registered");
        assert_eq!(row.status, Status::Implemented, "{id}");
        assert_eq!(row.oracle_record, record, "{id}");
        assert_eq!(row.hash_stage, id, "{id}: its hash stage is its own id");
        assert_eq!(row.stage, "layout");
        assert_eq!(row.geometry, Some("Point"), "{id}");
        assert!(row.scale_ceiling > 0, "{id}");
    }
    // Neither may borrow a record that does not speak for it: the d3-force stress arm
    // knows nothing of networkx's FA2, and the other way round.
    let other = |id: &str, record: &str| {
        assert!(
            !rows.iter().any(|r| r.id == id && r.oracle_record == record),
            "{id} must not be held to {record}"
        );
    };
    other("layout.forceatlas2", "stress");
    other("layout.force.barnes_hut", "oracle-fa2");
}

#[test]
fn a_row_serialises_to_the_section_8_keys_in_order() {
    let text = serde_json::to_string(&row(Status::Gated)).expect("serialisable");
    let keys = [
        "id",
        "tier",
        "stage",
        "geometry",
        "status",
        "oracle",
        "oracle_diff",
        "hash_4way",
        "scale_ceiling",
        "degradation",
        "ponytail",
        "complexity",
    ];
    let mut at = 0;
    for key in keys {
        let found = text[at..]
            .find(&format!("\"{key}\":"))
            .unwrap_or_else(|| panic!("{key}"));
        at += found;
    }
    assert!(
        !text.contains("functions")
            && !text.contains("hash_stage")
            && !text.contains("oracle_record"),
        "{text}"
    );
}

#[test]
fn status_serialises_to_the_four_ledger_words() {
    let words = [
        Status::Absent,
        Status::Stub,
        Status::Implemented,
        Status::Gated,
    ]
    .map(|s| serde_json::to_string(&s).expect("serialisable"));
    assert_eq!(
        words,
        ["\"absent\"", "\"stub\"", "\"implemented\"", "\"gated\""]
    );
}
