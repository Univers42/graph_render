//! The verdict against the ceilings, pinned per layout.

use super::*;

/// A full `oracle-spectral` result: every row of `SPECTRAL.ceilings` measured, so the
/// three 2D arms and the two 3D arms appended beside them are all judged. The 3D worsts
/// are the measured ones (`spectral_3d` 1.0296e-5 against its 1e-4, `pivot_mds_3d`
/// 3.69e-8 against its 1e-7 — `docs/measurements/p12-t4a.md`), not zeros: a fixture that
/// passed them by reporting nothing would not notice a ceiling moving under them.
fn result(spectral: (u64, f64), pivot: (u64, f64)) -> Value {
    json!({ "layouts": {
        "spectral": { "cases": spectral.0, "worst": spectral.1 },
        "pivot_mds": { "cases": pivot.0, "worst": pivot.1 },
        "spectral_3d": { "cases": 4, "worst": 1.0296e-5 },
        "pivot_mds_3d": { "cases": 4, "worst": 3.69e-8 },
    }})
}

#[test]
fn a_worst_at_or_under_its_ceiling_passes_and_records_it() {
    let (pass, functions) = judge(SPECTRAL.ceilings, &result((9, 1e-5), (4, 0.0))).expect("judged");
    assert!(pass);
    assert_eq!(functions["layout.spectral"]["cases"], 9);
    assert_eq!(functions["layout.spectral"]["unexplained"], 0);
    assert_eq!(functions["layout.mds.pivot"]["ceiling"], 1e-7);
    // The two 3D arms appended to the table, judged on the same pass: their own cases and
    // their own ceilings, so a ceiling widened or a case dropped on either is visible here.
    assert_eq!(functions["layout.spectral.3d"]["cases"], 4);
    assert_eq!(functions["layout.spectral.3d"]["unexplained"], 0);
    assert_eq!(functions["layout.mds.pivot.3d"]["ceiling"], 1e-7);
    assert_eq!(functions["layout.mds.pivot.3d"]["unexplained"], 0);
}

#[test]
fn a_worst_over_its_ceiling_or_no_component_fails_that_layout_only() {
    let (pass, functions) =
        judge(SPECTRAL.ceilings, &result((9, 1.1e-5), (4, 0.0))).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.spectral"]["unexplained"], 1);
    assert_eq!(functions["layout.mds.pivot"]["unexplained"], 0);
    let (pass, functions) = judge(SPECTRAL.ceilings, &result((9, 0.0), (0, 0.0))).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.mds.pivot"]["unexplained"], 1);
}

#[test]
fn a_result_without_a_measured_worst_is_refused() {
    assert!(judge(SPECTRAL.ceilings, &json!({ "layouts": {} })).is_err());
}

#[test]
fn closed_form_ceiling_covers_ring_spiral_and_bipartite() {
    // Every row of `CLOSED_FORM.ceilings` measured: the three 2D arms and the three 3D
    // arms appended beside them, each at its own ceiling. `random_3d` carries the
    // distribution metric's 0.5 rather than a coordinate tolerance, so it is stated at
    // 0.2 — inside its own ceiling, and far outside the 1e-7 the other two carry, so a
    // ceiling moved onto the wrong row would fail here.
    let result = json!({ "layouts": {
        "ring": { "cases": 5, "worst": 1e-6 },
        "spiral": { "cases": 5, "worst": 1e-7 },
        "bipartite": { "cases": 5, "worst": 2e-7 },
        "random_3d": { "cases": 5, "worst": 0.2 },
        "spiral_3d": { "cases": 5, "worst": 2.9802e-8 },
        "bipartite_3d": { "cases": 5, "worst": 2.9798e-8 },
    }});
    let (pass, functions) = judge(CLOSED_FORM.ceilings, &result).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.bipartite"]["unexplained"], 1);
    assert_eq!(functions["layout.spiral"]["unexplained"], 0);
    for id in [
        "layout.random.3d",
        "layout.spiral.3d",
        "layout.bipartite.3d",
    ] {
        assert_eq!(functions[id]["unexplained"], 0, "{id} is under its ceiling");
    }
    assert_eq!(functions["layout.random.3d"]["ceiling"], 0.5);
    assert_eq!(functions["layout.spiral.3d"]["ceiling"], 1e-7);
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
