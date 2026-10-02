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
use super::{find_row_by_id, hand, honest, recorded, without_osage_control};
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

/// `Status::Gated` needs two halves, and this is the test that holds the one this row can
/// lose. The oracle half is satisfied by `oracle-osage`; the other is a negative control that
/// went red on `layout.packing.osage` itself. Take that control away — an evidence set with a
/// current oracle record and no such control, which is what `honest()` builds — and the row is
/// refused with exactly one problem naming the missing half, even though the registry now
/// ships it as `gated`.
///
/// The row *is* `gated` now (`GM_MUTATE_PACKING_OSAGE_NODES` is the control, and
/// `a_red_control_on_the_rows_own_stage_is_the_whole_of_what_is_missing` below is its proof),
/// so this is no longer a statement about where the row stands. It is the refusal itself that
/// matters: a ledger that would accept a gated row with no control behind its own stage would
/// make the status mean nothing.
#[test]
fn osage_is_refused_gated_for_want_of_a_negative_control_on_its_stage() {
    let mut evidence = honest();
    without_osage_control(&mut evidence);
    let gated = find_row_by_id("layout.packing.osage");
    assert_eq!(
        gated.status,
        Status::Gated,
        "the registry ships this row gated, so the refusal below is the ledger refusing a row \
         it would otherwise accept"
    );
    let found = problems(std::slice::from_ref(&gated), &evidence);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].contains("no negative control backs the layout.packing.osage stage"),
        "the oracle half is satisfied; the control half is not: {found:?}"
    );
}

/// The control half is the *whole* of what was missing: with a control that went red on its
/// own stage the shipped row stands on its own oracle record with no other edit. This is
/// what the row needed from the hash gate, and naming it is the point of the test.
///
/// The control comes from the honest evidence set by its real record name —
/// `hashgate-control-packing-osage-nodes`, the one `Knob::PackingOsageNodes::record()`
/// returns — and nothing here forces the status, so what is under test is the row the
/// registry actually ships rather than a local copy with the answer written in.
#[test]
fn a_red_control_on_the_rows_own_stage_is_the_whole_of_what_is_missing() {
    let evidence = honest();
    let gated = find_row_by_id("layout.packing.osage");
    assert_eq!(
        problems(std::slice::from_ref(&gated), &evidence),
        Vec::<String>::new(),
        "a red control on the row's own stage is the whole of what is missing"
    );
    // The negative control on that control: one that went red on *another* stage says
    // nothing about this one, so the row is refused rather than borrowing a verdict. This
    // half is what `without_osage_control` is for — the honest set already holds the osage
    // control, and it is taken back out so that what stands in for it is the circle one.
    let mut elsewhere = honest();
    without_osage_control(&mut elsewhere);
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

/// The status stands where the evidence puts it: `layout.packing.osage` is the one Graphviz
/// row that earned `gated` (its differential passes and `GM_MUTATE_PACKING_OSAGE_NODES` is a
/// red control on its own stage), and the other five are untouched by any of this — each is
/// `implemented` for a measured reason of its own (twopi, neato and patchwork on `-Tplain`'s
/// five significant digits, circo on a qsort tie order in the oracle, fdp on the oracle
/// disagreeing with itself).
#[test]
fn the_graphviz_rows_keep_the_status_their_evidence_puts_them_at() {
    assert_eq!(
        graphviz_rows()
            .iter()
            .map(|r| (r.id, r.status))
            .collect::<Vec<(&str, Status)>>(),
        vec![
            ("layout.twopi", Status::Implemented),
            ("layout.packing.osage", Status::Gated),
            ("layout.circular.circo", Status::Implemented),
            ("layout.treemap.patchwork", Status::Implemented),
            ("layout.force.neato", Status::Implemented),
            ("layout.force.fdp", Status::Implemented),
        ]
    );
}

mod arms;
