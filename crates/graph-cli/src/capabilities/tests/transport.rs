//! The Phase 4 transport rows against the hash gate's own evidence: the 4-way stage, the
//! C20 tally recorded beside it, and the row that has no recorded gate yet. Split out of
//! the parent test module to keep both files under the house line limit.

use super::*;

/// The transport row is the one Phase 4 row whose two verdicts are the hash gate's own:
/// its 4-way stage and the per-seed tally that says the real ABI reached the shim's bytes.
#[test]
fn the_transport_row_reads_back_both_of_the_hash_gates_own_verdicts() {
    let evidence = honest();
    let rows = ledger(&evidence);
    let transport = find_row(&evidence, "transport.wasm.columnar");
    assert_eq!(transport.status, Status::Gated);
    assert_eq!(
        (transport.oracle_record, transport.hash_stage),
        ("wasm-transport", "transport.wasm.columnar")
    );
    assert_eq!(
        transport.hash_4way,
        "equal/1000 seeds (transport.wasm.columnar stage; negative control \
hashgate-control-grid-spacing red)"
    );
    assert_eq!(transport.oracle_diff, "byte-equal/1000 seeds (1000 cases)");
    assert_eq!(problems(&rows, &evidence), Vec::<String>::new());
    // The other Phase 4 row has no recorded gate of its own and must not claim otherwise.
    let sdk = find_row(&evidence, "sdk.js");
    assert_eq!(sdk.status, Status::Implemented);
    assert!(
        sdk.hash_4way.starts_with("not backed: "),
        "{}",
        sdk.hash_4way
    );
    assert!(
        sdk.oracle_diff.starts_with("not backed: "),
        "{}",
        sdk.oracle_diff
    );
}

/// Tallies that back nothing, each with the refusal it must produce: one short of every
/// seed, one about another stage, one about another reference, and one with no stage at
/// all.
fn bad_tallies() -> [(Value, &'static str); 4] {
    let honest = |equal| json!({ "stage": "transport.wasm.columnar", "reference": "layout.grid", "equal": equal });
    [
        (honest(999), "matched the shim on 999 of 1000 seeds"),
        (
            json!({ "stage": "topology", "reference": "layout.grid", "equal": 1000 }),
            "stage topology, not transport.wasm.columnar",
        ),
        (
            json!({ "stage": "transport.wasm.columnar", "reference": "topology", "equal": 1000 }),
            "reference topology, not layout.grid",
        ),
        (json!({ "equal": 1000 }), "no transport tally"),
    ]
}

#[test]
fn a_transport_tally_that_is_short_or_about_another_stage_backs_nothing() {
    for (tally, why) in bad_tallies() {
        let mut evidence = honest();
        evidence.hashgate.as_mut().expect("set")["transport"] = tally;
        let rows = ledger(&evidence);
        let transport = find_row(&evidence, "transport.wasm.columnar");
        assert!(
            transport.oracle_diff.contains(why),
            "{}",
            transport.oracle_diff
        );
        // The claim still stands on the row; `--check` is what refuses it, not a silent
        // downgrade to `implemented`.
        assert_eq!(transport.status, Status::Gated);
        let problems = problems(&rows, &evidence);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(
            problems[0].contains("transport.wasm.columnar: gated, but"),
            "{problems:?}"
        );
    }
}

#[test]
fn a_transport_tally_from_another_tree_is_refused_before_it_is_read() {
    let mut stale = honest();
    stale.fingerprint = "edited".into();
    assert!(
        find_row(&stale, "transport.wasm.columnar")
            .oracle_diff
            .contains("from another tree")
    );
}
