//! The Graphviz-engine rows' ledger claims: every engine's differential is read from the
//! record its own run wrote, resolved by the name that record carries rather than by an
//! arm the reader spells out per engine.
//!
//! `oracle_python::graphviz` keeps one table of the engines (`ENGINES`) and writes
//! `oracle-<engine>.json` for each. The ledger used to resolve a fixed list of record
//! names, so the six Graphviz arms — and every engine after them — read
//! `not backed: no oracle-<engine> record` forever, however often the gate was re-run. The
//! reader is now one rule over the records that exist, so a new engine is a name in that
//! table and nothing in this file changes.

use super::super::*;
use super::ledger::find_row;
use super::stages::equal_map;
use super::{find_row_by_id, hand, honest, recorded};
use serde_json::json;

/// The six Graphviz-armed rows, read off the registry rather than restated, so a row that
/// moved is a test that fails.
pub(super) fn graphviz_rows() -> Vec<Capability> {
    [
        "layout.twopi",
        "layout.packing.osage",
        "layout.circular.circo",
        "layout.treemap.patchwork",
        "layout.force.neato",
        "layout.force.fdp",
    ]
    .iter()
    .map(|id| find_row_by_id(id))
    .collect::<Vec<Capability>>()
}

/// The verdict a record a `tolerance: true` differential wrote reads back as.
pub(super) const BACKED: &str = "within measured ceiling of the oracle/1000 seeds (5 cases)";

/// Every Graphviz row's own record backs it, read by the name that record carries — the
/// arm `layout.packing.osage` could not reach before, and the reason a new engine needs no
/// edit here.
#[test]
fn every_graphviz_row_reads_its_own_record_by_name() {
    for row in graphviz_rows() {
        let mut evidence = honest();
        recorded(&mut evidence, row.oracle_record, row.id);
        assert_eq!(
            find_row(&evidence, row.id).oracle_diff,
            BACKED,
            "{}: its own record must back it by name",
            row.id
        );
    }
}

/// The negative control on the reader, first half: a record no run wrote reads as unbacked
/// rather than as a verdict, and the message names the record the row asked for.
#[test]
fn a_record_no_run_wrote_backs_nothing() {
    for row in graphviz_rows() {
        let mut evidence = honest();
        recorded(&mut evidence, row.oracle_record, row.id);
        assert_eq!(
            find_row(&evidence, row.id).oracle_diff,
            BACKED,
            "{}: the control must start from a backed row",
            row.id
        );
        evidence.by_name.remove(row.oracle_record);
        assert_eq!(
            find_row(&evidence, row.id).oracle_diff,
            format!("not backed: no {} record: run the gate", row.oracle_record),
            "{}: a record no run wrote",
            row.id
        );
    }
}

/// The negative control on the reader, second half: a record written on another tree is
/// refused before it is read, so an edit to any fingerprinted input voids it.
#[test]
fn a_graphviz_record_from_another_tree_is_refused() {
    for row in graphviz_rows() {
        let mut evidence = honest();
        recorded(&mut evidence, row.oracle_record, row.id);
        evidence.fingerprint = "edited".into();
        let moved = find_row(&evidence, row.id);
        assert!(
            moved.oracle_diff.contains(&format!(
                "{} record is from another tree",
                row.oracle_record
            )),
            "{}: {}",
            row.id,
            moved.oracle_diff
        );
    }
}

/// One way a record stops backing a row: the edit that makes it so, and the refusal the row
/// must then read back as.
type Case = (fn(&mut Evidence), &'static str);

/// The ways a record that is present and current on this tree still backs nothing.
fn short_or_red() -> Vec<Case> {
    vec![
        (
            |e| e.by_name.get_mut("oracle-osage").expect("set")["seeds"] = json!(999),
            "oracle-osage ran 999 seeds, need 1000",
        ),
        (
            |e| e.by_name.get_mut("oracle-osage").expect("set")["pass"] = json!(false),
            "oracle-osage did not pass",
        ),
        (
            |e| {
                e.by_name.get_mut("oracle-osage").expect("set")["functions"]["layout.packing.osage"]
                    ["unexplained"] = json!(1)
            },
            "oracle-osage: layout.packing.osage has unexplained mismatches",
        ),
    ]
}

/// A record that is present and current still backs nothing when the run behind it was
/// short, went red, or never compared this row's function.
#[test]
fn a_short_a_red_or_an_empty_graphviz_run_backs_nothing() {
    for (edit, why) in short_or_red() {
        let mut evidence = honest();
        recorded(&mut evidence, "oracle-osage", "layout.packing.osage");
        edit(&mut evidence);
        assert_eq!(
            find_row(&evidence, "layout.packing.osage").oracle_diff,
            format!("not backed: {why}"),
            "osage"
        );
    }
    let mut blind = honest();
    recorded(&mut blind, "oracle-osage", "layout.packing.osage");
    blind.by_name.get_mut("oracle-osage").expect("set")["functions"]["layout.packing.osage"]["cases"] =
        json!(0);
    let read = find_row(&blind, "layout.packing.osage");
    assert!(
        read.oracle_diff
            .contains("ran no layout.packing.osage case"),
        "{}",
        read.oracle_diff
    );
}

/// A record's *name* is not enough: the row also asks for a case on its own function, so a
/// record filed under another engine's layout cannot stand in. The two arms of the control
/// are one test because the honest half is what makes the refusal meaningful.
#[test]
fn a_record_name_without_the_rows_own_function_backs_nothing() {
    let mut evidence = honest();
    recorded(&mut evidence, "oracle-osage", "layout.twopi");
    assert_eq!(
        find_row(&evidence, "layout.packing.osage").oracle_diff,
        "not backed: oracle-osage ran no layout.packing.osage case",
        "a record comparing another layout is not this row's record"
    );
    recorded(&mut evidence, "oracle-osage", "layout.packing.osage");
    assert_eq!(
        find_row(&evidence, "layout.packing.osage").oracle_diff,
        BACKED,
        "the same record, comparing this row's own function, backs it"
    );
    // And the same rule across engines: `layout.twopi` is not backed by osage's run.
    let read = find_row(&evidence, "layout.twopi");
    assert!(
        read.oracle_diff.contains("no oracle-twopi record"),
        "{}",
        read.oracle_diff
    );
}

/// `layout.packing.osage` is the one Graphviz row whose differential now has a record the
/// ledger resolves, and it is still `implemented`. The remaining reason is on the **hash**
/// side, not the oracle side, and this is the test that says so: `Status::Gated` needs a
/// 4-way verdict on the row's own stage *and* a negative control that went red on that
/// same stage, and no control covers `layout.packing.osage` — the per-stage knob for a
/// Graphviz engine is deliberately not in the table
/// (`scripts/orch/rows/p13-gv1-osage.rows:23-31`), because `hashgate/knob.rs` is shared
/// with the parallel engine jobs.
#[test]
fn osage_is_refused_gated_for_want_of_a_negative_control_on_its_stage() {
    let mut evidence = honest();
    recorded(&mut evidence, "oracle-osage", "layout.packing.osage");
    let mut gated = find_row_by_id("layout.packing.osage");
    gated.status = Status::Gated;
    let found = problems(std::slice::from_ref(&gated), &evidence);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("no negative control backs the layout.packing.osage stage"),
        "the oracle half is satisfied; the control half is not: {found:?}"
    );
}

/// The control half is the *whole* of what is missing: give the row a control that went red
/// on its own stage and it stands on its own oracle record with no other edit. This is what
/// the row needs from the hash gate, and naming it is the point of the test.
#[test]
fn a_red_control_on_the_rows_own_stage_is_the_whole_of_what_is_missing() {
    let mut evidence = honest();
    recorded(&mut evidence, "oracle-osage", "layout.packing.osage");
    evidence.controls.push((
        "hashgate-control-packing-osage-nodes",
        Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": false,
            "equal": equal_map(1000, &["layout.packing.osage"]),
        })),
    ));
    let mut gated = find_row_by_id("layout.packing.osage");
    gated.status = Status::Gated;
    assert_eq!(
        problems(std::slice::from_ref(&gated), &evidence),
        Vec::<String>::new(),
        "a red control on the row's own stage is the whole of what is missing"
    );
    // The negative control on that control: one that went red on *another* stage says
    // nothing about this one, so the row is refused rather than borrowing a verdict.
    let mut elsewhere = honest();
    recorded(&mut elsewhere, "oracle-osage", "layout.packing.osage");
    elsewhere.controls.push((
        "hashgate-control-packing-circle-nodes",
        Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": false,
            "equal": equal_map(1000, &["layout.packing.circle"]),
        })),
    ));
    let found = problems(std::slice::from_ref(&gated), &elsewhere);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("no negative control backs the layout.packing.osage stage"),
        "a control on another stage must not stand in: {found:?}"
    );
}

/// The status stands where the evidence puts it, and the other five Graphviz rows are
/// untouched by any of this: each is `implemented` for a measured reason of its own
/// (twopi, neato and patchwork on `-Tplain`'s five significant digits, circo on a qsort
/// tie order in the oracle, fdp on the oracle disagreeing with itself).
#[test]
fn the_graphviz_rows_stay_implemented() {
    assert_eq!(
        graphviz_rows()
            .iter()
            .map(|r| (r.id, r.status))
            .collect::<Vec<(&str, Status)>>(),
        vec![
            ("layout.twopi", Status::Implemented),
            ("layout.packing.osage", Status::Implemented),
            ("layout.circular.circo", Status::Implemented),
            ("layout.treemap.patchwork", Status::Implemented),
            ("layout.force.neato", Status::Implemented),
            ("layout.force.fdp", Status::Implemented),
        ]
    );
}

mod arms;
