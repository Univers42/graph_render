use super::*;
use std::collections::BTreeMap;

/// A green `--check` used to be silent about the difference between a row with evidence and
/// a row with none, and more than half this ledger ships with none — so "0 problems" read
/// the same either way. Every row now prints which of the three states it is in.
#[test]
fn every_row_says_whether_a_recorded_run_backs_it() {
    let evidence = honest();
    let rows = ledger(&evidence);
    let of = |id: &str| {
        rows.iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("no {id} row"))
    };
    assert_eq!(
        how_backed(of("topology.index")),
        "backed by a recorded run on this tree"
    );
    assert_eq!(
        how_backed(of("analysis.components.weak")),
        "no evidence: nothing recorded backs it"
    );
    let mut half = row(Status::Implemented);
    half.oracle_diff = "byte-equal/1000 seeds (5 cases)".into();
    half.hash_4way = "not backed: no hashgate record: run the gate".into();
    assert_eq!(
        how_backed(&half),
        "partly backed: one verdict, not both",
        "one verdict of two is its own state, not 'backed'"
    );
    let with_none = rows
        .iter()
        .filter(|row| how_backed(row) == "no evidence: nothing recorded backs it")
        .count();
    assert!(
        with_none > 20,
        "{with_none} of {} rows ship with no evidence at all, and now say so",
        rows.len()
    );
    for row in &rows {
        assert!(!how_backed(row).is_empty(), "{}: a stated state", row.id);
    }
}

/// The ceilings table is read by `--check` and not only by the flag that names it. A
/// missing doc used to be an absent check: `--check` exited 0 over a tree with no
/// `phase09-ceilings.md` at all, and a row the table names but the ledger does not have
/// was invisible unless `--ceilings-measured` was passed as well.
#[test]
fn the_ceilings_table_is_read_by_check_and_a_missing_doc_is_a_finding() {
    let evidence = honest();
    let rows = ledger(&evidence);
    let real = &crate::runner::workspace_root().join(CEILINGS_DOC);
    assert!(
        check_findings(&rows, &evidence, real).is_empty(),
        "the tree's own doc is clean"
    );
    let missing = std::env::temp_dir().join(format!("gm-no-ceilings-{}", std::process::id()));
    let found = check_findings(&rows, &evidence, &missing);
    assert!(
        found
            .iter()
            .any(|f| f.contains(CEILINGS_DOC) && f.contains("No such file")),
        "{found:?}"
    );
    let dir = std::env::temp_dir().join(format!("gm-bad-ceilings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let bad = dir.join("phase09-ceilings.md");
    std::fs::write(
        &bad,
        "| id | declared | measured |\n|---|---:|---|\n| layout.nope | 1 | 2 |\n",
    )
    .expect("writable");
    let found = check_findings(&rows, &evidence, &bad);
    assert!(
        found
            .iter()
            .any(|f| f.contains("layout.nope") && f.contains("the ledger does not have")),
        "{found:?}"
    );
    std::fs::remove_dir_all(&dir).expect("removable");
}

#[test]
fn every_registered_row_stands_on_honest_evidence_and_reads_it_back() {
    let evidence = honest();
    let rows = ledger(&evidence);
    let of = |id: &str| {
        rows.iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("no {id} row"))
            .clone()
    };
    assert_eq!(problems(&rows, &evidence), Vec::<String>::new());
    let topology = of("topology.index");
    assert_eq!(
        topology.hash_4way,
        "equal/1000 seeds (topology stage; negative control hashgate-control-reference-degree red)"
    );
    assert_eq!(topology.oracle_diff, "byte-equal/1000 seeds (25 cases)");
    let grid = of("layout.grid");
    assert_eq!(
        (grid.stage, grid.geometry, grid.complexity),
        ("layout", Some("Point"), "O(n)")
    );
    assert_eq!(
        grid.hash_4way,
        "equal/1000 seeds (layout.grid stage; negative control hashgate-control-grid-spacing red)"
    );
    assert_eq!(grid.oracle_diff, "byte-equal/1000 seeds (7 cases)");
    let dag = of("layout.dag.sugiyama");
    assert_eq!((dag.geometry, dag.scale_ceiling), (Some("Point"), 200_000));
    assert_eq!(
        dag.hash_4way,
        "equal/1000 seeds (layout.dag.sugiyama stage; negative control \
hashgate-control-sugiyama-layer-spacing red)"
    );
    assert_eq!(dag.oracle_diff, "byte-equal/1000 seeds (9 cases)");
}

/// One row of the ledger over `evidence`, by id.
pub(super) fn find_row(evidence: &Evidence, id: &str) -> Capability {
    ledger(evidence)
        .into_iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no {id} row"))
}

#[test]
fn the_grid_row_stands_only_on_its_own_control_and_its_roundtrip_record() {
    let grid = || vec![find_row_by_id("layout.grid")];
    let mut evidence = honest();
    evidence.controls.truncate(1);
    let blind = problems(&grid(), &evidence);
    assert_eq!(blind.len(), 1, "{blind:?}");
    assert!(
        blind[0]
            .contains("hashgate-control-reference-degree did not go red on the layout.grid stage"),
        "{blind:?}"
    );
    let mut evidence = honest();
    evidence.by_name.remove("roundtrip");
    let unchecked = problems(&grid(), &evidence);
    assert!(
        unchecked[0].contains("no roundtrip record"),
        "{unchecked:?}"
    );
    let mut evidence = honest();
    evidence.by_name.get_mut("roundtrip").expect("set")["functions"]["layout.grid"]["unexplained"] =
        json!(2);
    let wrong = problems(&grid(), &evidence);
    assert!(
        wrong[0].contains("roundtrip: layout.grid has unexplained"),
        "{wrong:?}"
    );
}

#[test]
fn without_records_every_gated_row_is_refused_twice() {
    let bare = Evidence {
        fingerprint: "tree".into(),
        hashgate: None,
        controls: vec![],
        by_name: BTreeMap::new(),
    };
    let rows = ledger(&bare);
    // **Derived from the registry, not spelled out.** Every problem here comes from a gated
    // row — `problems` evaluates the two verdicts for a gated row and for no other status —
    // and each such row yields exactly two, so the count *is* twice the gated rows. The
    // literal this replaces was one that had to be edited by hand whenever a row was
    // promoted, and a hand-edited literal is a place a row can be promoted and the number
    // quietly left behind. Deriving it ties the count to the thing it counts.
    let gated = rows.iter().filter(|r| r.status == Status::Gated).count();
    assert!(gated > 0, "the registry gates something");
    assert_eq!(problems(&rows, &bare).len(), 2 * gated);
    // By id, not by position: the first row happens to be `topology.index` today, and a
    // registry entry inserted above it would leave this test passing on a row it never
    // meant to read.
    let index = rows
        .iter()
        .find(|r| r.id == "topology.index")
        .expect("topology.index is a row");
    assert!(
        index
            .hash_4way
            .starts_with("not backed: no hashgate record")
    );
    assert!(
        index
            .oracle_diff
            .starts_with("not backed: no oracle-diff record")
    );
}
