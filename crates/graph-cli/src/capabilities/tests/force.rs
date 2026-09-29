//! The force rows' ledger claims: `gated` only on their own hash and their own oracle.

use super::super::*;
use super::{hand, honest};
use serde_json::json;

/// One real force row out of the registry, promoted to `gated` — the claim the phase
/// prompt forbids making without both oracles behind it.
fn force_row(id: &str) -> Capability {
    let mut row = registry()
        .into_iter()
        .find(|r| r.id == id)
        .expect("registered");
    row.status = Status::Gated;
    row
}

/// A force row may be promoted to `gated` only with a 4-way hash **and** its own
/// oracle differential, both on this tree and both over 1000 seeds. Every way of
/// shortchanging it is refused, including the tempting one: borrowing the *other*
/// force layout's record, which names a different library and a different metric.
#[test]
fn a_force_row_is_refused_gated_without_its_own_hash_and_its_own_oracle() {
    for (id, record, other_record) in [
        ("layout.force.barnes_hut", "stress", "oracle-fa2"),
        ("layout.forceatlas2", "oracle-fa2", "stress"),
    ] {
        let promoted = force_row(id);
        assert!(
            problems(std::slice::from_ref(&promoted), &honest()).is_empty(),
            "{id}: honest evidence backs it"
        );
        let gone = |e: &mut Evidence| match id {
            "layout.force.barnes_hut" => e.stress = None,
            _ => e.fa2 = None,
        };
        let no_oracle = refused_like(&promoted, gone);
        assert_eq!(no_oracle.len(), 1, "{id}: {no_oracle:?}");
        assert!(
            no_oracle[0].contains(&format!("no {record} record")),
            "{id}: {no_oracle:?}"
        );
        let no_hash = problems(std::slice::from_ref(&promoted), &{
            let mut e = honest();
            e.hashgate = None;
            e
        });
        assert!(
            no_hash[0].contains("no hashgate record"),
            "{id}: {no_hash:?}"
        );
        // With only the *other* force layout's record present, this row's own
        // `oracle_diff` must read as unbacked rather than borrowing a passing verdict
        // from a record that names a different library and a different metric.
        let only_other = ledger(&{
            let mut e = honest();
            match id {
                "layout.force.barnes_hut" => {
                    e.stress = None;
                    e.fa2 = Some(json!({
                        "fingerprint": "tree", "seeds": 1000, "pass": true,
                        "functions": { "layout.forceatlas2": hand(4) }
                    }));
                }
                _ => {
                    e.fa2 = None;
                    e.stress = Some(json!({
                        "fingerprint": "tree", "seeds": 1000, "pass": true,
                        "functions": { "layout.force.barnes_hut": hand(4) }
                    }));
                }
            }
            e
        });
        let mine = only_other.iter().find(|r| r.id == id).expect("row");
        assert!(
            mine.oracle_diff.contains(&format!("no {record} record")),
            "{id}: {other_record} must not stand in for {record}: {}",
            mine.oracle_diff
        );
    }
}

/// `problems` over one given row against evidence `edit` has changed.
fn refused_like(row: &Capability, edit: impl FnOnce(&mut Evidence)) -> Vec<String> {
    let mut evidence = honest();
    edit(&mut evidence);
    problems(std::slice::from_ref(row), &evidence)
}
