use super::*;

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
    evidence.roundtrip = None;
    let unchecked = problems(&grid(), &evidence);
    assert!(
        unchecked[0].contains("no roundtrip record"),
        "{unchecked:?}"
    );
    let mut evidence = honest();
    evidence.roundtrip.as_mut().expect("set")["functions"]["layout.grid"]["unexplained"] = json!(2);
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
        oracle: None,
        roundtrip: None,
        layouts: None,
        stress: None,
        fa2: None,
        spectral: None,
    };
    let rows = ledger(&bare);
    assert_eq!(problems(&rows, &bare).len(), 34);
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
