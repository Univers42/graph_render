//! The verdict against the ceilings, pinned per layout.

use super::*;

/// A spectral result holding `(key, cases, worst)` per arm, so a caller states only the arms
/// it cares about and every arm the judge reads is one the caller wrote.
fn result(arms: &[(&str, u64, f64)]) -> Value {
    let layouts: serde_json::Map<String, Value> = arms
        .iter()
        .map(|&(key, cases, worst)| (key.to_string(), json!({ "cases": cases, "worst": worst })))
        .collect();
    json!({ "layouts": layouts })
}

/// The four spectral arms at their real measured worsts, over the 1000 gate seeds on this
/// tree: `docs/measurements/p12-3d-oracles.md` restates the run.
const MEASURED_SPECTRAL: [(&str, u64, f64); 4] = [
    ("spectral", 984, 7.178_463_314_258_969e-6),
    ("pivot_mds", 996, 3.691_204_508_715_629e-8),
    ("spectral_3d", 982, 1.029_618_482_075_63e-5),
    ("pivot_mds_3d", 990, 4.837_812_772_606_753e-8),
];

/// **Every arm of the differential is stated, or the judge refuses.** The two rows below
/// each perturb ONE arm of [`MEASURED_SPECTRAL`] and leave the other three alone, because
/// `judge` reads a missing key as "no measured worst" and refuses the whole result — which
/// is the behaviour that stops a differential quietly shrinking to the arms that still pass.
#[test]
fn a_worst_at_or_under_its_ceiling_passes_and_records_it() {
    let mut arms = MEASURED_SPECTRAL;
    arms[0].2 = 1e-5;
    let (pass, functions) = judge(SPECTRAL.ceilings, &result(&arms)).expect("judged");
    assert!(pass);
    assert_eq!(functions["layout.spectral"]["cases"], 984);
    assert_eq!(functions["layout.spectral"]["unexplained"], 0);
    assert_eq!(functions["layout.mds.pivot"]["ceiling"], 1e-7);
    assert_eq!(functions["layout.spectral3d"]["ceiling"], 1e-4);
}

#[test]
fn a_worst_over_its_ceiling_or_no_component_fails_that_layout_only() {
    let mut arms = MEASURED_SPECTRAL;
    arms[0].2 = 1.1e-5;
    let (pass, functions) = judge(SPECTRAL.ceilings, &result(&arms)).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.spectral"]["unexplained"], 1);
    assert_eq!(functions["layout.mds.pivot"]["unexplained"], 0);

    // And the other direction: a layout the harness compared no component of.
    arms[0].2 = MEASURED_SPECTRAL[0].2;
    arms[1].1 = 0;
    let (pass, functions) = judge(SPECTRAL.ceilings, &result(&arms)).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.mds.pivot"]["unexplained"], 1);
    assert_eq!(functions["layout.spectral"]["unexplained"], 0);
}

/// **Every measured spectral worst, 2-D and 3-D, against the ceilings this tree declares.**
/// If a ceiling is ever widened to make this pass, the measurement is what says so, and
/// `docs/measurements/p12-3d-oracles.md` has to be rewritten with it — which is why the
/// numbers are literals here and not read back from a result file this suite produced.
///
/// The 3-D arms are in the same row on purpose: `layout.spectral3d` and `layout.spectral`
/// are one algorithm at two widths through one run function, so a ceiling that covers one
/// and not the other would mean the gap moved rather than the layout.
#[test]
fn every_measured_spectral_worst_stays_under_its_own_ceiling() {
    let measured = result(&MEASURED_SPECTRAL);
    let (pass, functions) = judge(SPECTRAL.ceilings, &measured).expect("judged");
    let mut lines: Vec<String> = SPECTRAL
        .ceilings
        .iter()
        .map(|&(id, key, ceiling)| {
            format!(
                "{id}: worst {:.4e} over {} cases, ceiling {ceiling:.0e}",
                measured["layouts"][key]["worst"].as_f64().expect("a worst"),
                functions[id]["cases"].as_u64().expect("cases"),
            )
        })
        .collect();
    lines.sort();
    assert!(pass, "{lines:#?}");
    assert_eq!(functions.len(), SPECTRAL.ceilings.len());
}

/// **The control for the row above, and it is per arm.** Every 3-D worst is pushed just over
/// its own ceiling and must fail THAT layout and no other, so a ceiling that was widened by
/// an order of magnitude would still be caught here: `spectral_3d` is the one arm whose
/// ceiling (1e-4) sits above its measured worst (1.03e-5) by nearly a decade, and pushing
/// its worst to 2e-4 must turn it red while `pivot_mds_3d` stays green at 4.8e-8.
#[test]
fn one_3d_worst_over_its_ceiling_fails_only_that_arm() {
    let mut arms = MEASURED_SPECTRAL;
    arms[2].2 = 2e-4;
    let (pass, functions) = judge(SPECTRAL.ceilings, &result(&arms)).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.spectral3d"]["unexplained"], 1);
    for id in ["layout.spectral", "layout.mds.pivot", "layout.mds.pivot3d"] {
        assert_eq!(functions[id]["unexplained"], 0, "{id} is unaffected");
    }
}

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
fn closed_form_ceiling_covers_ring_spiral_and_bipartite() {
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

/// **Every measured closed-form worst, 2-D and 3-D, against the ceilings this tree declares**,
/// over the 1000 gate seeds: `docs/measurements/p12-3d-oracles.md` restates the run.
///
/// The point of the 3-D rows here is the pair of ceilings rather than the passes. `ring`'s
/// 2.65e-7 sits under 1e-6 and `spiral_3d`'s 2.38e-7 sits under the *same* 1e-6 while
/// `spiral`'s 2.98e-8 sits under 1e-7 — the two coordinate arms differ by one power of ten
/// only because they draw at `scale = 5.0`. Reading them as one family and taking the
/// smaller ceiling would turn this row red, which is the point: the numbers say the ceiling
/// tracks the layout's magnitude, not the arm's name.
#[test]
fn every_measured_closed_form_worst_stays_under_its_own_ceiling() {
    let measured = json!({ "layouts": {
        "ring": { "cases": 1000, "worst": 2.653_535_747_798_585_5e-7 },
        "spiral": { "cases": 1000, "worst": 2.980_183_511_080_980_5e-8 },
        "bipartite": { "cases": 1000, "worst": 2.980_083_901_871_211e-8 },
        "random_3d": { "cases": 1000, "worst": 0.303_003_460_168_838_5 },
        "spiral_3d": { "cases": 1000, "worst": 2.384_184_796_255_795e-7 },
        "bipartite_3d": { "cases": 1000, "worst": 1.191_999_814_409_427_9e-7 },
    }});
    let (pass, functions) = judge(CLOSED_FORM.ceilings, &measured).expect("judged");
    let mut lines: Vec<String> = CLOSED_FORM
        .ceilings
        .iter()
        .map(|&(id, key, ceiling)| {
            format!(
                "{id}: worst {:.4e} over {} cases, ceiling {ceiling:.0e}",
                measured["layouts"][key]["worst"].as_f64().expect("a worst"),
                functions[id]["cases"].as_u64().expect("cases"),
            )
        })
        .collect();
    lines.sort();
    assert!(pass, "{lines:#?}");
    assert_eq!(functions.len(), CLOSED_FORM.ceilings.len());
}

/// **The control for the row above, and it is the distribution arm.** `layout.random.3d`'s
/// ceiling is 0.5 — a sampling-noise floor, not a coordinate tolerance — so the only way to
/// know it still bites is to push a worst past it and watch the row go red while both
/// coordinate arms stay green at their `f32` floors. A ceiling widened to 1.0 here would
/// pass this test and fail the one above, which is the pair's whole point.
#[test]
fn a_skewed_random_3d_stream_fails_only_its_own_row() {
    let measured = json!({ "layouts": {
        "ring": { "cases": 1000, "worst": 2.653_535_747_798_585_5e-7 },
        "spiral": { "cases": 1000, "worst": 2.980_183_511_080_980_5e-8 },
        "bipartite": { "cases": 1000, "worst": 2.980_083_901_871_211e-8 },
        "random_3d": { "cases": 1000, "worst": 0.6 },
        "spiral_3d": { "cases": 1000, "worst": 2.384_184_796_255_795e-7 },
        "bipartite_3d": { "cases": 1000, "worst": 1.191_999_814_409_427_9e-7 },
    }});
    let (pass, functions) = judge(CLOSED_FORM.ceilings, &measured).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.random.3d"]["unexplained"], 1);
    for id in [
        "layout.circular.ring",
        "layout.spiral",
        "layout.bipartite",
        "layout.basic3d.spiral",
        "layout.bipartite_3d",
    ] {
        assert_eq!(functions[id]["unexplained"], 0, "{id} is unaffected");
    }
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
