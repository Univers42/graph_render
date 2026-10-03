//! A failing run never replaces a passing record, unless that record is another tree's.
//!
//! Split out of `tests.rs` by the house's 300-line limit.

use super::*;

#[test]
fn a_failing_run_never_overwrites_a_passing_record() {
    let dir = scratch("no-clobber");
    let passed = serde_json::json!({ "seeds": 1000, "pass": true });
    let path = recorded(write_to(&dir, "hashgate", passed, "f".into()));
    let kept = std::fs::read_to_string(&path).expect("readable");
    // A control run, a short sweep, or a run on a tree that has since regressed:
    // each writes the same record name, and each must leave the passing one standing.
    let why = refused(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    assert!(why.contains("hashgate"), "{why}");
    assert_eq!(
        std::fs::read_to_string(&path).expect("readable"),
        kept,
        "the passing record is byte-for-byte what it was"
    );
    // The reverse is allowed and is the point of a gate: a passing run replaces a
    // failing one of the same name, which is what re-running for a fix must do.
    recorded(write_to(
        &dir,
        "roundtrip",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    recorded(write_to(
        &dir,
        "roundtrip",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "f".into(),
    ));
    assert_eq!(
        read_from(&dir, "roundtrip").expect("readable"),
        Some(serde_json::json!({
            "seeds": 2000, "pass": true, "gate": "roundtrip", "fingerprint": "f"
        }))
    );
    // A record with no boolean `pass` is not a verdict and is not written at all, so it
    // can neither replace a passing record nor stand as one. (The oracle harnesses used
    // to write exactly that shape; every writer in `graph-cli` sets a real boolean today,
    // so this is a refusal and not a lost record.)
    assert!(
        matches!(
            write_to(
                &dir,
                "oracle-diff",
                serde_json::json!({ "seeds": 1000 }),
                "f".into()
            ),
            Outcome::Failed(_)
        ),
        "a body with no `pass` is not a record"
    );
    for not_a_verdict in [
        serde_json::json!({ "pass": null }),
        serde_json::json!({ "pass": "false" }),
    ] {
        assert!(
            matches!(
                write_to(&dir, "oracle-diff", not_a_verdict.clone(), "f".into()),
                Outcome::Failed(_)
            ),
            "{not_a_verdict} is not a verdict"
        );
        assert_eq!(
            read_from(&dir, "oracle-diff").expect("readable"),
            None,
            "nothing was written for {not_a_verdict}"
        );
    }
    // The control, in the same guard: a real failing record *does* replace a failing one.
    recorded(write_to(
        &dir,
        "oracle-diff",
        serde_json::json!({ "seeds": 1000, "pass": false }),
        "f".into(),
    ));
    recorded(write_to(
        &dir,
        "oracle-diff",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "f".into(),
    ));
    assert_eq!(
        read_from(&dir, "oracle-diff").expect("readable"),
        Some(serde_json::json!({
            "seeds": 8, "pass": false, "gate": "oracle-diff", "fingerprint": "f"
        }))
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// A passing record another tree left behind is already void to the ledger, so it must
/// not stand in the way of this tree's failing run: refusing the write would leave
/// exactly that void record as the file `capabilities --check` reads.
#[test]
fn a_stale_passing_record_from_another_tree_does_not_block_this_trees_run() {
    let dir = scratch("other-tree");
    recorded(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "other".into(),
    ));
    recorded(write_to(
        &dir,
        "hashgate",
        serde_json::json!({ "seeds": 8, "pass": false }),
        "this".into(),
    ));
    assert_eq!(
        read_from(&dir, "hashgate")
            .expect("readable")
            .expect("present"),
        serde_json::json!({ "seeds": 8, "pass": false, "gate": "hashgate", "fingerprint": "this" }),
        "this tree's own run is recorded; the other tree's record is gone"
    );
    // Same tree, and the rule still holds: a failing run never replaces a passing record.
    let kept = scratch("same-tree");
    recorded(write_to(
        &kept,
        "hashgate",
        serde_json::json!({ "seeds": 2000, "pass": true }),
        "this".into(),
    ));
    assert!(
        refused(write_to(
            &kept,
            "hashgate",
            serde_json::json!({ "seeds": 8, "pass": false }),
            "this".into(),
        ))
        .contains("hashgate")
    );
    for dir in [&dir, &kept] {
        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
