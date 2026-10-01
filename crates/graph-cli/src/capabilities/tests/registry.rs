//! The registry's own shape: coverage, unique ids, the record each row's two verdicts are
//! read from, and the ledger row's serialised form (`prompt.md` §8). Split out of the
//! parent test module to keep both files under the house line limit.

use super::*;
use std::collections::BTreeSet;

/// Phase 8's bundling and style rows. `post.route.grid` was the only POST row the ledger
/// carried, so `post.bundle.fdeb`, `post.bundle.mingle` and the four `post.style.*` rows
/// had no row at all — each a registered, unit-tested capability with no statement of what
/// it costs or what it owes, which is what `prompt.md` §8 asks a ledger to have.
#[test]
fn every_post_capability_is_a_row_beside_the_routing_one() {
    let rows = registry();
    let post: Vec<&Capability> = rows.iter().filter(|r| r.stage == "post").collect();
    let mut ids: Vec<&str> = post.iter().map(|r| r.id).collect();
    let mut want = vec![
        "post.route.grid",
        "post.bundle.fdeb",
        "post.bundle.mingle",
        "post.style.straight",
        "post.style.orthogonal",
        "post.style.quadratic",
        "post.style.bezier",
    ];
    // The style rows follow `post::styles::STYLES`'s own order, which is the order that
    // module documents ("the order the ledger lists them"), so compare as sets and pin
    // the count: a duplicate id is caught by the registry's own uniqueness test.
    ids.sort_unstable();
    want.sort_unstable();
    assert_eq!(
        ids, want,
        "one row per registered POST capability: none missing, none invented"
    );
    assert_eq!(post.len(), 7, "seven POST rows, not a duplicate of one");
    for row in post {
        assert_eq!(
            row.status,
            Status::Implemented,
            "{}: implemented, never gated without a recorded differential",
            row.id
        );
        assert!(row.geometry.is_some(), "{}: names its geometry", row.id);
        assert!(row.scale_ceiling > 0, "{}: a stated ceiling", row.id);
        for (field, value) in [
            ("oracle", row.oracle),
            ("complexity", row.complexity),
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
        ] {
            assert!(!value.trim().is_empty(), "{}: {field} is empty", row.id);
        }
    }
}

/// Each POST row restates the metadata its own module in `graph-core` declares, rather
/// than a second and looser answer: the ceiling, oracle, complexity, degradation and
/// Ponytail are read across, so a change to the module's `META` and a stale ledger row
/// cannot disagree.
#[test]
fn a_post_row_carries_the_metadata_its_own_module_declares() {
    use graph_core::post::{fdeb, mingle, styles};
    let rows = registry();
    let of = |id: &str| {
        rows.iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("{id} is a row"))
    };
    for (id, meta) in [(fdeb::ID, &fdeb::META), (mingle::ID, &mingle::META)] {
        let row = of(id);
        assert_eq!(
            (
                row.scale_ceiling,
                row.oracle,
                row.complexity,
                row.degradation,
                row.ponytail
            ),
            (
                meta.scale_ceiling,
                meta.oracle,
                meta.complexity,
                meta.degradation,
                meta.ponytail
            ),
            "{id}"
        );
    }
    for style in styles::STYLES {
        let row = of(style.id);
        assert_eq!(
            (
                row.scale_ceiling,
                row.oracle,
                row.complexity,
                row.degradation,
                row.ponytail
            ),
            (
                style.meta.scale_ceiling,
                style.meta.oracle,
                style.meta.complexity,
                style.meta.degradation,
                style.meta.ponytail
            ),
            "{}",
            style.id
        );
    }
}

/// The style rows differ in exactly one field, `edges`, and the ledger says so: a style
/// never emits two geometry kinds, so each row names one, and no row may name another's.
#[test]
fn each_style_row_names_the_geometry_kind_its_own_style_emits() {
    use graph_contract::geometry::EdgeGeometryKind;
    let name = |kind: EdgeGeometryKind| match kind {
        EdgeGeometryKind::Line => "Line",
        EdgeGeometryKind::Polyline => "Polyline",
        _ => "Curve",
    };
    let rows = registry();
    for style in graph_core::post::styles::STYLES {
        let row = rows
            .iter()
            .find(|r| r.id == style.id)
            .unwrap_or_else(|| panic!("{} is a row", style.id));
        assert_eq!(row.geometry, Some(name(style.meta.edges)), "{}", style.id);
    }
}

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
        } else if r.id == "layout.force.barnes_hut" || r.id == "layout.force.yifan_hu" {
            ("stress", r.id, Status::Implemented)
        } else if r.id == "layout.forceatlas2" {
            ("oracle-fa2", r.id, Status::Implemented)
        } else if r.id == "layout.force.spring" {
            ("oracle-spring", r.id, Status::Implemented)
        } else if r.id == "layout.circular.hierarchy" {
            // A closed form with a SciGraphs-arm differential, `implemented` rather than
            // `gated` for the reason `unproven.rs` gives: the ledger resolves no such
            // record, so a gated row could only ever read back a refusal.
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
        ("layout.force.spring", "oracle-spring"),
        ("layout.circular.hierarchy", "oracle-circular-hierarchy"),
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
    // The two FR ports share a metric and share no code, and the two closed forms are
    // compared by different arms entirely: nobody may be measured by another's run.
    other("layout.force.spring", "oracle-fa2");
    other("layout.force.spring", "stress");
    other("layout.circular.hierarchy", "oracle-closed-form");
    other("layout.circular.hierarchy", "oracle-spring");
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
