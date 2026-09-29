use super::verdict::{Evidence, MIN_SEEDS};
use super::*;
use serde_json::{Value, json};

mod force;
mod refusals;
mod registry;
mod sugiyama;
mod transport;

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

/// Every hashgate stage's key, in `hashgate::STAGES` order, so this fixture's `equal`
/// maps can be built at the same shape a real record has, without importing the
/// hashgate module just for the constant.
const STAGES: [&str; 12] = [
    "topology",
    "layout.grid",
    "layout.tree.tidy",
    "layout.treemap.squarified",
    "layout.circular.radial",
    "layout.packing.circle",
    "layout.spectral",
    "layout.mds.pivot",
    "layout.force.barnes_hut",
    "layout.forceatlas2",
    "layout.dag.sugiyama",
    "transport.wasm.columnar",
];

/// A hashgate-shaped `equal` map: `seeds` for every stage, except `diverged`'s, at `0`.
fn equal_map(seeds: u64, diverged: &[&str]) -> Value {
    let map: serde_json::Map<String, Value> = STAGES
        .iter()
        .map(|&stage| {
            let count = if diverged.contains(&stage) { 0 } else { seeds };
            (stage.to_owned(), json!(count))
        })
        .collect();
    Value::Object(map)
}

/// One `{cases, declared: 0, unexplained: 0}` function entry.
fn hand(cases: u64) -> Value {
    json!({ "cases": cases, "declared": 0, "unexplained": 0 })
}

/// One control's record: red, over `seeds` seeds, diverging exactly `diverged`'s stages.
fn control(name: &'static str, diverged: &[&str]) -> (&'static str, Option<Value>) {
    (
        name,
        Some(json!({
            "fingerprint": "tree", "seeds": 8, "pass": false,
            "equal": equal_map(8, diverged)
        })),
    )
}

/// The controls: reference degree (topology, treemap — it reads node weight), grid
/// spacing (grid alone), layer spacing (the layered drawing alone), node count —
/// restricted here to the layouts no other control reaches, since reference degree and
/// grid spacing already back topology/grid/treemap on their own (a real run may show it
/// diverging those too; the ledger only needs one control per stage to hold) — and one
/// control per force layout, which reach nothing else at all.
fn honest_controls() -> Vec<(&'static str, Option<Value>)> {
    vec![
        control(
            "hashgate-control-reference-degree",
            &["topology", "layout.treemap.squarified"],
        ),
        control(
            "hashgate-control-grid-spacing",
            &["layout.grid", "transport.wasm.columnar"],
        ),
        control(
            "hashgate-control-sugiyama-layer-spacing",
            &["layout.dag.sugiyama"],
        ),
        control(
            "hashgate-control-node-count",
            &[
                "layout.tree.tidy",
                "layout.treemap.squarified",
                "layout.circular.radial",
                "layout.packing.circle",
                "layout.spectral",
                "layout.mds.pivot",
            ],
        ),
        control("hashgate-control-force-theta", &["layout.force.barnes_hut"]),
        control(
            "hashgate-control-fa2-scaling-ratio",
            &["layout.forceatlas2"],
        ),
    ]
}

fn honest() -> Evidence {
    let functions: serde_json::Map<String, Value> =
        COVERED.iter().map(|f| ((*f).to_owned(), hand(5))).collect();
    Evidence {
        fingerprint: "tree".into(),
        hashgate: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "equal": equal_map(1000, &[]),
            "transport": {
                "stage": "transport.wasm.columnar", "reference": "layout.grid",
                "equal": 1000
            }
        })),
        controls: honest_controls(),
        oracle: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "functions": functions
        })),
        roundtrip: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": {
                "layout.grid": hand(7),
                "layout.circular.radial": hand(6),
                "layout.packing.circle": hand(5),
                "layout.dag.sugiyama": hand(9),
            }
        })),
        layouts: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": {
                "layout.tree.tidy": hand(9),
                "layout.treemap.squarified": hand(11),
            }
        })),
        stress: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": { "layout.force.barnes_hut": hand(4) }
        })),
        fa2: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true,
            "functions": { "layout.forceatlas2": hand(4) }
        })),
        spectral: Some(json!({
            "fingerprint": "tree", "seeds": 1000, "pass": true, "tolerance": true,
            "functions": {
                "layout.spectral": hand(12),
                "layout.mds.pivot": hand(13),
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
    assert_eq!(rows.len(), 32);
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
    let dag = &rows[17];
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

/// One row of the ledger over `evidence`, by id.
pub(super) fn find_row(evidence: &Evidence, id: &str) -> Capability {
    ledger(evidence)
        .into_iter()
        .find(|r| r.id == id)
        .unwrap_or_else(|| panic!("no {id} row"))
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
        layouts: None,
        stress: None,
        fa2: None,
        spectral: None,
    };
    let rows = ledger(&bare);
    assert_eq!(problems(&rows, &bare).len(), 34);
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

/// Phase 9's three scale rows, and what they are allowed to claim: `implemented`, never
/// `gated` — nothing hashes them yet (the hash gate's stage list is outside this phase's
/// envelope), and a row that claimed `gated` without that evidence would be refused by
/// [`problems`] for exactly the right reason.
#[test]
fn the_scale_stage_publishes_three_implemented_rows_with_every_required_field() {
    let rows = registry();
    let scale: Vec<&Capability> = rows.iter().filter(|r| r.stage == "scale").collect();
    let ids: Vec<&str> = scale.iter().map(|r| r.id).collect();
    assert_eq!(ids, ["scale.lod", "scale.simplify", "scale.adaptive"]);
    for row in scale {
        assert_eq!(row.status, Status::Implemented, "{}", row.id);
        assert!(row.scale_ceiling > 0, "{}", row.id);
        for (field, value) in [
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
            ("complexity", row.complexity),
            ("oracle", row.oracle),
        ] {
            assert!(!value.trim().is_empty(), "{}: {field} is empty", row.id);
        }
    }
}

/// The ledger grew by exactly the three scale rows (on top of develop's post row), and no row lost its evidence.
#[test]
fn the_ledger_is_the_registry_plus_the_scale_rows_and_still_stands() {
    let evidence = honest();
    let rows = ledger(&evidence);
    assert_eq!(rows.len(), 32);
    assert_eq!(problems(&rows, &evidence), Vec::<String>::new());
}

/// `--ceilings-measured`: a row the table covers must carry a number, an id the ledger
/// does not have is a finding, and a row the table says nothing about is *counted* as
/// still reasoned rather than counted as measured.
#[test]
fn the_ceilings_table_is_read_as_measured_unmeasured_and_unknown() {
    let rows = vec![
        {
            let mut first = row(Status::Implemented);
            first.id = "layout.grid";
            first
        },
        {
            let mut second = row(Status::Implemented);
            second.id = "layout.other";
            second
        },
    ];
    let table = "| id | declared | measured |\n|---|---:|---|\n\
                 | layout.grid | 100000 | 220 |\n\
                 | layout.other | 500 | not measured |\n";
    let findings = ceiling_findings(&rows, table);
    assert_eq!(
        findings,
        vec![
            "layout.other: the table's measured cell is `not measured`, not a number (declared 9700000)"
                .to_string(),
        ]
    );
    assert_eq!(
        ceiling_coverage(&rows, table),
        (1, 1),
        "one measured, one still reasoned"
    );
    assert!(
        ceiling_findings(
            &rows,
            "| id | declared | measured |\n| layout.nope | 1 | 2 |\n"
        )
        .iter()
        .any(|f| f.contains("layout.nope") && f.contains("the ledger does not have"))
    );
}
