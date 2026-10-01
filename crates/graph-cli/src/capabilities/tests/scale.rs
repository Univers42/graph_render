use super::*;

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

/// The ledger grew by the three scale rows, Phase 10's four ingest rows and Phase 8's six
/// bundling and style rows (on top of develop's `post.route.grid`), and no row lost its
/// evidence. The count is pinned by *id*, not by index: `prompt.md` §8's note is that row
/// indices move whenever a registry entry is inserted before them, so the assertion names
/// the rows rather than counting past them.
#[test]
fn the_ledger_is_the_registry_plus_the_scale_rows_and_still_stands() {
    let evidence = honest();
    let rows = ledger(&evidence);
    let ids: Vec<&str> = rows.iter().map(|r| r.id).collect();
    for id in [
        "analysis.depth",
        "post.bundle.fdeb",
        "post.bundle.mingle",
        "post.style.straight",
        "post.style.orthogonal",
        "post.style.quadratic",
        "post.style.bezier",
    ] {
        assert!(ids.contains(&id), "{id} is a row");
    }
    assert_eq!(
        rows.len(),
        50,
        "42 before analysis.depth, 25 before the seven post.* rows, and 49 before \
         layout.twopi"
    );
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
