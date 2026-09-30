//! The verdict against the ceilings, pinned per layout.

use super::*;

fn result(spectral: (u64, f64), pivot: (u64, f64)) -> Value {
    json!({ "layouts": {
        "spectral": { "cases": spectral.0, "worst": spectral.1 },
        "pivot_mds": { "cases": pivot.0, "worst": pivot.1 },
    }})
}

#[test]
fn a_worst_at_or_under_its_ceiling_passes_and_records_it() {
    let (pass, functions) = judge(SPECTRAL.ceilings, &result((9, 1e-5), (4, 0.0))).expect("judged");
    assert!(pass);
    assert_eq!(functions["layout.spectral"]["cases"], 9);
    assert_eq!(functions["layout.spectral"]["unexplained"], 0);
    assert_eq!(functions["layout.mds.pivot"]["ceiling"], 1e-7);
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
    let result = json!({ "layouts": {
        "ring": { "cases": 5, "worst": 1e-6 },
        "spiral": { "cases": 5, "worst": 1e-7 },
        "bipartite": { "cases": 5, "worst": 2e-7 },
    }});
    let (pass, functions) = judge(CLOSED_FORM.ceilings, &result).expect("judged");
    assert!(!pass);
    assert_eq!(functions["layout.bipartite"]["unexplained"], 1);
    assert_eq!(functions["layout.spiral"]["unexplained"], 0);
}
