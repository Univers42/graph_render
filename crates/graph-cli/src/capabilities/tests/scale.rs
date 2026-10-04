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
    // Every registered layout is a row, by id: the ledger is generated from the registry,
    // so a layout that is registered but not published is a row no consumer can see. The
    // row count is deliberately not asserted — it moves whenever a registry entry or a
    // stage row is added, and the by-id checks above are the claim worth making.
    for id in graph_core::registry::LAYOUTS.iter().map(|layout| layout.id) {
        assert!(ids.contains(&id), "{id} is a row");
    }
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
        vec![format!(
            "layout.other: the table's measured cell is `not measured`, not a number (declared {MAX_SCALE_CEILING})"
        )]
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

/// A `measured` cell is the **largest N this phase ran**, so it sits below the ceiling its
/// row declares. A cell above it is a mislabelled column or a wrong declaration, and the
/// cell used to be checked for being digits and nothing else — `measured` was read only to
/// interpolate `declared` into the failure message, so either read as "measured" and
/// `--ceilings-measured` exited 0.
#[test]
fn a_measured_cell_above_the_declared_ceiling_is_refused() {
    let mut measured = row(Status::Implemented);
    measured.id = "layout.grid";
    measured.scale_ceiling = 1_000;
    let rows = vec![measured];
    let under = "| id | declared | measured |\n|---|---:|---|\n| layout.grid | 1000 | 220 |\n";
    assert_eq!(ceiling_findings(&rows, under), Vec::<String>::new());
    let over = "| id | declared | measured |\n|---|---:|---|\n| layout.grid | 1000 | 100000000 |\n";
    let found = ceiling_findings(&rows, over);
    assert!(
        found
            .iter()
            .any(|f| f.contains("measured 100000000 is above the declared ceiling 1000")),
        "{found:?}"
    );
    assert_eq!(
        ceiling_coverage(&rows, over),
        (1, 0),
        "and it is still a number, so still counted"
    );
}

/// The columns are read by the **header's own names**. `cells[1]`/`cells[3]` happened to be
/// `id`/`measured` in the one table this phase wrote, so a reordered table, or a row written
/// without its leading pipe, read a different column — and a plain integer in the wrong
/// column was accepted as a measurement.
#[test]
fn the_columns_are_read_by_the_headers_names_not_by_position() {
    let rows = vec![row(Status::Implemented)];
    let reordered = "| measured | how | id |\n|---|---:|---|\n| 220 | a note | topology.index |\n";
    assert_eq!(
        ceiling_coverage(&rows, reordered),
        (1, 0),
        "`id` is column 3 here, not column 1"
    );
    let missing = "| how | id |\n|---:|---|\n| a note | topology.index |\n";
    assert!(
        ceiling_findings(&rows, missing)
            .iter()
            .any(|f| f.contains("no `measured` one")),
        "a table with no `measured` column says so rather than reading `how` as one"
    );
    let unled = "topology.index | 9700000 | 220 |\n";
    assert!(
        ceiling_findings(&rows, unled)
            .iter()
            .any(|f| f.contains("without its leading pipe")),
        "a row missing its leading pipe used to shift every cell by one"
    );
}
