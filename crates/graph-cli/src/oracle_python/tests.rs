//! The verdict against the ceilings, pinned per layout.

use super::*;

mod measured;

/// A result the harness wrote under `--break` carries the case it broke: its mismatches are
/// the control's, not a defect, so nothing in it may be read as a pass. A result with no
/// `broken` key at all is every other differential's, and says nothing.
#[test]
fn a_deliberately_broken_result_is_never_a_pass() {
    let rows = json!({ "emitted": 6, "cases": 6, "exact": 3, "ties": 3, "worst": 0.0 });
    let honest = json!({ "layouts": { "apply_budget": rows }, "broken": null });
    assert!(unbroken(&honest));
    assert!(!unbroken(&json!({ "broken": "lod.budget.tie" })));
    assert!(
        !unbroken(&json!({ "layouts": { "apply_budget": rows }, "broken": "lod.budget.tie" })),
        "a broken run cannot be the one the ledger records"
    );
}

#[test]
fn a_result_without_a_measured_worst_is_refused() {
    assert!(judge(SPECTRAL.ceilings, &json!({ "layouts": {} })).is_err());
}

/// A differential over hand-built cases states how many it emitted, and every one of them
/// has to have been compared: `cases` are the ones the harness read and judged, split into
/// the ones the two arms returned the same bytes for (`exact`) and the ones whose order the
/// reference cannot decide (`ties`, compared against the rule). A run that read a case and
/// judged none of it would otherwise report a shorter, passing table.
#[test]
fn a_case_the_harness_never_compared_fails_its_layout() {
    let rows = |cases: u64, exact: u64, ties: u64| json!({ "emitted": 6, "cases": cases, "exact": exact, "ties": ties, "worst": 0.0 });
    let with = |row: Value| {
        json!({ "layouts": { "apply_budget": row,
                             "frustum_cull_spheres": { "emitted": 2, "cases": 2,
                                                        "exact": 2, "ties": 0, "worst": 0.0 },
                             "build_coarse_level": { "emitted": 4, "cases": 4,
                                                     "exact": 4, "ties": 0, "worst": 0.0 } } })
    };
    assert!(
        judge(SCALE.ceilings, &with(rows(6, 3, 3)))
            .expect("judged")
            .0,
        "every emitted case judged"
    );
    for bad in [rows(5, 3, 3), rows(6, 4, 3), rows(6, 2, 2), rows(7, 3, 3)] {
        let (pass, _) = judge(SCALE.ceilings, &with(bad.clone())).expect("judged");
        assert!(!pass, "{bad} counts a case twice or not at all");
    }
}

#[test]
fn closed_form_ceiling_covers_every_row_it_declares() {
    let result = json!({ "layouts": {
        "ring": { "cases": 5, "worst": 1e-6 },
        "spiral": { "cases": 5, "worst": 1e-7 },
        "bipartite": { "cases": 5, "worst": 2e-7 },
        "random_3d": { "cases": 5, "worst": 0.2 },
        "spiral_3d": { "cases": 5, "worst": 2.0e-7 },
        "bipartite_3d": { "cases": 5, "worst": 5.0e-7 },
    }});
    let (pass, functions) = judge(CLOSED_FORM.ceilings, &result).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.bipartite"]["unexplained"], 1);
    assert_eq!(functions["layout.spiral"]["unexplained"], 0);
    assert_eq!(functions["layout.random.3d"]["unexplained"], 0);
    assert_eq!(functions["layout.basic3d.spiral"]["unexplained"], 0);
    assert_eq!(functions["layout.bipartite_3d"]["unexplained"], 0);
}

/// Two failure modes, one per direction, and neither touches a passing layout: a layout
/// with no `ours` column has no case to judge, and a worst over its own ceiling fails.
/// The numbers are chosen against the real ceilings (`CEILING_TIGHT` 10, `CEILING_LOOSE`
/// 100) rather than a value that would pass under any of them, so the test cannot quietly
/// agree with a ceiling that was widened to make it green.
#[test]
fn an_igraph_layout_with_no_ours_column_is_not_run_and_fails() {
    let result = json!({ "layouts": {
        "fruchterman_reingold": { "cases": 0, "worst": 0.0 },
        "kamada_kawai": { "cases": 5, "worst": 1.2 },
        "drl": { "cases": 5, "worst": 12.0 },
        "lgl": { "cases": 5, "worst": 2.0 },
        "davidson_harel": { "cases": 5, "worst": 0.9 },
        "graphopt": { "cases": 5, "worst": 1.0 },
    }});
    let (pass, functions) = judge(IGRAPH.ceilings, &result).expect("judged");
    assert!(!pass);
    let mut failed: Vec<&str> = functions
        .iter()
        .filter(|(_, v)| v["unexplained"] == 1)
        .map(|(k, _)| k.as_str())
        .collect();
    failed.sort_unstable();
    assert_eq!(
        failed,
        ["layout.force.drl", "layout.force.fruchterman_reingold"]
    );
}

/// The measured `ours / igraph` worsts of the six layouts, over 100 seeds on 2026-09-29,
/// judged against the ceilings this tree declares. If a ceiling is ever widened to make
/// this pass, the measurement is what says so, and
/// `docs/measurements/p12-igraph-ceilings.md` has to be rewritten with it.
#[test]
fn every_measured_igraph_worst_stays_under_its_own_ceiling() {
    let result = result_from_measured_worsts();
    let (pass, functions) = judge(IGRAPH.ceilings, &result).expect("judged");
    let mut ceiling_lines: Vec<String> = IGRAPH
        .ceilings
        .iter()
        .map(|&(id, key, ceiling)| {
            let row = functions
                .get(id)
                .unwrap_or_else(|| panic!("{id} is judged"))
                .clone();
            format!(
                "{key}: worst {:.2} over {} cases, ceiling {ceiling:.0e}",
                result["layouts"][key]["worst"].as_f64().expect("a worst"),
                row["cases"].as_u64().expect("cases"),
            )
        })
        .collect();
    ceiling_lines.sort();
    assert!(pass, "{ceiling_lines:#?}");
    assert_eq!(functions.len(), IGRAPH.ceilings.len());
}

/// The `ours / igraph` worst per layout, as measured over 100 seeds on 2026-09-29
/// (`docs/measurements/p12-igraph-ceilings.md` restates the run and its floor).
fn result_from_measured_worsts() -> Value {
    json!({ "layouts": {
        "fruchterman_reingold": { "cases": 100, "worst": 1.30 },
        "kamada_kawai": { "cases": 100, "worst": 1.35 },
        "drl": { "cases": 100, "worst": 3.98 },
        "lgl": { "cases": 100, "worst": 2.13 },
        "davidson_harel": { "cases": 100, "worst": 51.91 },
        "graphopt": { "cases": 100, "worst": 15.39 },
    }})
}
