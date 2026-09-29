use super::verdict::{Evidence, MIN_SEEDS};
use super::*;
use serde_json::{Value, json};

mod registry;

/// The 17 oracle functions of `prompt.md` §7.4, plus the H4 and H9 arms.
const COVERED: [&str; 19] = [
    "indexModel",
    "emptyModel",
    "nodesEqual",
    "makeRecordNodeId",
    "makeNoteNodeId",
    "makeTagNodeId",
    "makeEdgeId",
    "parseNodeId",
    "applyDegreeWeights",
    "edgeKindFromType",
    "diffGraph",
    "isEmptyPatch",
    "edgesEqual",
    "deriveLegend",
    "neighborhood",
    "neighborhoodEdges",
    "buildSyntheticModel",
    "hashString",
    "layoutGroups",
];

fn honest() -> Evidence {
    let functions: serde_json::Map<String, Value> = COVERED
        .iter()
        .map(|f| {
            (
                (*f).to_owned(),
                json!({ "cases": 5, "declared": 0, "unexplained": 0 }),
            )
        })
        .collect();
    let control = |topology: u32, grid: u32, dag: u32| {
        Some(json!({
            "fingerprint": "tree", "seeds": 8, "pass": false,
            "equal": {
                "topology": topology, "layout.grid": grid, "layout.dag.sugiyama": dag
            }
        }))
    };
    Evidence {
        fingerprint: "tree".into(),
        hashgate: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "equal": {
                "topology": 1000, "layout.grid": 1000, "layout.dag.sugiyama": 1000
            }
        })),
        controls: vec![
            ("hashgate-control-reference-degree", control(0, 8, 8)),
            ("hashgate-control-grid-spacing", control(8, 0, 8)),
            ("hashgate-control-sugiyama-layer-spacing", control(8, 8, 0)),
        ],
        oracle: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "functions": functions
        })),
        roundtrip: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": {
                "layout.grid": { "cases": 7, "declared": 0, "unexplained": 0 },
                "layout.dag.sugiyama": { "cases": 9, "declared": 0, "unexplained": 0 }
            }
        })),
    }
}

fn row(status: Status) -> Capability {
    let mut row = registry().remove(0);
    row.status = status;
    row
}

#[test]
fn every_registered_row_stands_on_honest_evidence_and_reads_it_back() {
    let evidence = honest();
    let rows = ledger(&evidence);
    assert_eq!(rows.len(), 10);
    assert_eq!(problems(&rows, &evidence), Vec::<String>::new());
    assert_eq!(
        rows[0].hash_4way,
        "equal/1000 seeds (topology stage; negative control hashgate-control-reference-degree red)"
    );
    assert_eq!(rows[0].oracle_diff, "byte-equal/1000 seeds (25 cases)");
    let grid = &rows[8];
    assert_eq!(
        (grid.id, grid.stage, grid.geometry, grid.complexity),
        ("layout.grid", "layout", Some("Point"), "O(n)")
    );
    assert_eq!(
        grid.hash_4way,
        "equal/1000 seeds (layout.grid stage; negative control hashgate-control-grid-spacing red)"
    );
    assert_eq!(grid.oracle_diff, "byte-equal/1000 seeds (7 cases)");
    let dag = &rows[9];
    assert_eq!(
        (dag.id, dag.geometry, dag.scale_ceiling),
        ("layout.dag.sugiyama", Some("Point"), 200_000)
    );
    assert_eq!(
        dag.hash_4way,
        "equal/1000 seeds (layout.dag.sugiyama stage; negative control \
hashgate-control-sugiyama-layer-spacing red)"
    );
    assert_eq!(dag.oracle_diff, "byte-equal/1000 seeds (9 cases)");
}

#[test]
fn the_sugiyama_row_stands_only_on_its_own_control() {
    let dag = || vec![registry().remove(9)];
    let mut evidence = honest();
    evidence.controls.truncate(2);
    let blind = problems(&dag(), &evidence);
    assert_eq!(blind.len(), 1, "{blind:?}");
    assert!(
        blind[0].contains(
            "hashgate-control-grid-spacing did not go red on the layout.dag.sugiyama stage"
        ),
        "{blind:?}"
    );
    let mut evidence = honest();
    evidence.roundtrip.as_mut().expect("set")["functions"]["layout.dag.sugiyama"]["cases"] =
        json!(0);
    let empty = problems(&dag(), &evidence);
    assert!(
        empty[0].contains("ran no layout.dag.sugiyama case"),
        "{empty:?}"
    );
}

#[test]
fn the_grid_row_stands_only_on_its_own_control_and_its_roundtrip_record() {
    let grid = || vec![registry().remove(8)];
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
    };
    let rows = ledger(&bare);
    assert_eq!(problems(&rows, &bare).len(), 20);
    assert!(
        rows[0]
            .hash_4way
            .starts_with("not backed: no hashgate record")
    );
    assert!(
        rows[0]
            .oracle_diff
            .starts_with("not backed: no oracle-diff record")
    );
}

fn refused(edit: impl FnOnce(&mut Evidence)) -> Vec<String> {
    let mut evidence = honest();
    edit(&mut evidence);
    problems(&[row(Status::Gated)], &evidence)
}

#[test]
fn a_record_from_another_tree_or_too_few_seeds_is_refused() {
    let stale = refused(|e| e.fingerprint = "edited".into());
    assert_eq!(stale.len(), 2, "{stale:?}");
    assert!(stale[0].contains("from another tree"), "{stale:?}");
    let short = refused(|e| e.hashgate.as_mut().expect("set")["seeds"] = json!(MIN_SEEDS - 1));
    assert!(short[0].contains("ran 999 seeds, need 1000"), "{short:?}");
    let short = refused(|e| e.oracle.as_mut().expect("set")["seeds"] = json!(999));
    assert!(short[0].contains("oracle-diff ran 999 seeds"), "{short:?}");
}

#[test]
fn a_failed_run_a_short_stage_or_a_green_control_is_refused() {
    let red = refused(|e| e.hashgate.as_mut().expect("set")["pass"] = json!(false));
    assert!(red[0].contains("hashgate did not pass"), "{red:?}");
    let stage = refused(|e| e.hashgate.as_mut().expect("set")["equal"]["topology"] = json!(999));
    assert!(
        stage[0].contains("stage topology not 4-way equal"),
        "{stage:?}"
    );
    let control = refused(|e| degree_control(e)["pass"] = json!(true));
    assert!(
        control[0].contains("hashgate-control-reference-degree did not go red;"),
        "{control:?}"
    );
    for (topology, why) in [(json!(8), "8 of 8 equal"), (json!(null), "no count")] {
        let blind = refused(|e| degree_control(e)["equal"]["topology"] = topology.clone());
        assert!(
            blind[0].contains("no negative control backs the topology stage"),
            "{why}: {blind:?}"
        );
        assert!(
            blind[0].contains("reference-degree did not go red on the topology stage"),
            "{why}: {blind:?}"
        );
    }
    let control = refused(|e| e.controls[0].1 = None);
    assert!(
        control[0].contains("no hashgate-control-reference-degree record"),
        "{control:?}"
    );
    let stale = refused(|e| degree_control(e)["fingerprint"] = json!("old"));
    assert!(stale[0].contains("from another tree"), "{stale:?}");
    let none = refused(|e| e.controls.clear());
    assert!(none[0].ends_with("no control is registered"), "{none:?}");
    let oracle = refused(|e| e.oracle.as_mut().expect("set")["pass"] = json!(false));
    assert!(oracle[0].contains("oracle-diff did not pass"), "{oracle:?}");
}

fn degree_control(e: &mut Evidence) -> &mut Value {
    e.controls[0].1.as_mut().expect("set")
}

fn functions(e: &mut Evidence) -> &mut Value {
    &mut e.oracle.as_mut().expect("set")["functions"]
}

#[test]
fn a_function_without_cases_or_with_an_unexplained_mismatch_is_refused() {
    let none = refused(|e| functions(e)["emptyModel"]["cases"] = json!(0));
    assert!(none[0].contains("ran no emptyModel case"), "{none:?}");
    let wrong = refused(|e| functions(e)["indexModel"]["unexplained"] = json!(1));
    assert!(wrong[0].contains("indexModel has unexplained"), "{wrong:?}");
    let mut evidence = honest();
    functions(&mut evidence)["layoutGroups"]["declared"] = json!(3);
    assert_eq!(
        ledger(&evidence)[0].oracle_diff,
        "byte-equal/1000 seeds (25 cases, 3 declared divergences)"
    );
}

#[test]
fn empty_required_fields_zero_ceiling_and_duplicate_ids_are_refused() {
    let mut bare = row(Status::Implemented);
    bare.ponytail = " ";
    bare.degradation = "";
    bare.scale_ceiling = 0;
    assert_eq!(problems(&[bare], &honest()).len(), 3);
    let twice = [row(Status::Stub), row(Status::Stub)];
    assert_eq!(
        problems(&twice, &honest()),
        ["topology.index: duplicate id"]
    );
}
