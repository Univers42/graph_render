//! The registry's own shape: coverage, unique ids, and the ledger row's serialised form
//! (`prompt.md` §8). Split out of the parent test module to keep both files under the
//! house line limit; the assertions are unchanged.

use super::*;
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
    assert!(rows.iter().all(|r| r.status == Status::Gated));
    for r in &rows {
        let expected = if r.id.starts_with("topology.") {
            ("oracle-diff", "topology")
        } else if r.id == "layout.tree.tidy" || r.id == "layout.treemap.squarified" {
            ("oracle-layouts", r.id)
        } else {
            ("roundtrip", r.id)
        };
        assert_eq!((r.oracle_record, r.hash_stage), expected, "{}", r.id);
    }
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
