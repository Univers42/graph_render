//! The two rules that decide whether a row may skip a comparison: the pin rules that let
//! `GRAPHVIZ_DOT` pass unmeasured, and the rules that stop the exemption being a blank cheque.

use super::harness::{baseline_for, dir, judge, ok_row, pin};
use crate::oracle_python::conformance::baseline::{Baseline, row};
use serde_json::json;

/// A row the arm could not measure keeps its cell and **fails**. This is the test that stops
/// a skipped measurement from reading as a pass.
#[test]
fn a_row_that_could_not_be_measured_fails_rather_than_skipping() {
    let tag = "unmeasurable";
    let (ok, why) = judge(
        tag,
        baseline_for("SPRING_3D", tag),
        &json!({ "metrics": "not run: no reference coordinates for this fixture" }),
        &pin(&dir(tag), "motor", "SPRING_3D"),
        &pin(&dir(tag), "ref", "SPRING_3D"),
    );
    assert!(!ok);
    assert!(why.contains("not run"), "{why}");
}

/// A row with no motor layout is a fact about the tree, and it passes **only** because it
/// still says so **and** both pairs of bytes still match. The first test pins that the reason
/// has to be `reference-absent`; the second that the bytes are still the point.
///
/// **The reference sha is compared on this row too**, which is the defect this pair exists to
/// hold shut: the motor file is the empty one, so its digest is a constant and a judge that
/// checked the bytes only *after* the `not run` question would never reach the reference half
/// and would pass whatever the reference arm produced, or did not.
#[test]
fn a_row_with_no_motor_layout_passes_only_while_it_still_says_so() {
    let tag = "no-motor";
    // The cause is the point: the pass-through is gated on `reference-absent`, and a baseline
    // saying `convention` must fail even with both pairs of bytes correct.
    let declared: &'static Baseline = Box::leak(Box::new(row(
        "GRAPHVIZ_DOT",
        Box::leak(pin(&dir(tag), "motor", "GRAPHVIZ_DOT").into_boxed_str()),
        Box::leak(pin(&dir(tag), "ref", "GRAPHVIZ_DOT").into_boxed_str()),
        "",
        f64::INFINITY,
        "shape",
        "reference-absent",
    )));
    let empty = pin(&dir(tag), "motor", "GRAPHVIZ_DOT");
    let dot_bytes = pin(&dir(tag), "ref", "GRAPHVIZ_DOT");
    let unmeasurable = json!({ "metrics": "not run: no motor layout for this name" });

    let (ok, why) = judge_pins(tag, declared, &unmeasurable, &empty, &dot_bytes);
    assert!(ok, "{why}");

    // The reference digest changes and the row notices, which is the check an earlier ordering
    // made unreachable.
    let (ok, why) = judge_pins(tag, declared, &unmeasurable, &empty, "other");
    assert!(!ok, "an unmeasurable row skipped its reference sha");
    assert!(why.contains("reference bytes"), "{why}");

    // …and the pass-through is gated on the **cause**, not only on the words. The same
    // unmeasurable row, pinned as a measurable one, must fail: otherwise any row could opt out
    // of its reference comparison by claiming `not run`.
    let pinned_measurable: &'static Baseline = Box::leak(Box::new(row(
        "GRAPHVIZ_DOT",
        Box::leak(empty.clone().into_boxed_str()),
        Box::leak(dot_bytes.clone().into_boxed_str()),
        "",
        f64::INFINITY,
        "shape",
        "convention",
    )));
    let (ok, why) = judge(tag, pinned_measurable, &unmeasurable, &empty, &dot_bytes);
    assert!(!ok, "an unmeasurable row passed while pinned as measurable");
    assert!(why.contains("not run"), "{why}");
}

/// **`GRAPHVIZ_FDP`, the one row whose reference is not reproducible between runs.** Its
/// reference sha is empty **with a reason**, and the row passes anyway — because the motor's
/// bytes and the measured shape are still gated, and the log says the reference was not pinned.
///
/// This is the test that stops the exemption becoming a blank cheque: a note with an empty
/// reference sha passes, a note whose *motor* sha moved still fails, and a row with no note
/// and an empty reference sha fails.
#[test]
fn an_unreproducible_reference_passes_but_still_gates_the_motor() {
    let tag = "unreproducible";
    let motor = pin(&dir(tag), "motor", "GRAPHVIZ_FDP");
    let declared: &'static Baseline = Box::leak(Box::new(row(
        "GRAPHVIZ_FDP",
        Box::leak(motor.clone().into_boxed_str()),
        "",
        "the engine moves between runs",
        1.0,
        "shape",
        "rng",
    )));
    let (ok, why) = judge_pins(tag, declared, &ok_row(), &motor, "anything");
    assert!(ok, "{why}");
    assert!(why.contains("reference not pinned"), "{why}");

    let (ok, why) = judge_pins(tag, declared, &ok_row(), "other", "anything");
    assert!(
        !ok,
        "an unreproducible reference must not un-gate the motor"
    );
    assert!(why.contains("motor bytes"), "{why}");

    let silent: &'static Baseline = Box::leak(Box::new(row(
        "SPRING_3D",
        Box::leak(pin(&dir(tag), "motor", "SPRING_3D").into_boxed_str()),
        "",
        "",
        1.0,
        "shape",
        "rng",
    )));
    let (ok, why) = judge(
        tag,
        silent,
        &ok_row(),
        &pin(&dir(tag), "motor", "SPRING_3D"),
        "anything",
    );
    assert!(!ok, "an unpinned reference with no reason must fail");
    assert!(why.contains("reference sha is not pinned"), "{why}");
}

/// **An empty pin never passes**, even when everything else about the row is complete and both
/// shas are present and agree. Comparing an empty string with an empty string would make the
/// absence of a measurement read as agreement.
#[test]
fn an_empty_pinned_motor_sha_is_never_agreement() {
    let unpinned: &'static Baseline = Box::leak(Box::new(row(
        "SPRING_3D",
        "",
        "",
        "",
        f64::INFINITY,
        "shape",
        "reference-absent",
    )));
    let tag = "empty-pin";
    let (ok, why) = judge(
        tag,
        unpinned,
        &ok_row(),
        "",
        &pin(&dir(tag), "ref", "SPRING_3D"),
    );
    assert!(!ok, "an unpinned row must never pass");
    assert!(why.contains("motor sha is not pinned"), "{why}");
}

/// [`judge`] under this module's own name, so the two test files read as siblings.
pub(super) fn judge_pins(
    tag: &str,
    base: &'static Baseline,
    got: &serde_json::Value,
    motor: &str,
    reference: &str,
) -> (bool, String) {
    judge(tag, base, got, motor, reference)
}
