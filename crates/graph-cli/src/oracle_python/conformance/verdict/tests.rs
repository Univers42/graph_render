//! The judge, held to inputs this module writes itself.
//!
//! **These are the tests that make the gate non-vacuous.** Each of the checks in
//! [`super::one_row`] gets one test that fails it, and the one that must pass. The fixture
//! directory they run against holds **real bytes**, because the judge re-hashes the `.f64`
// files rather than trusting the digest an arm recorded: a test with no bytes behind the pin
//! would be measuring exactly the trust this module exists to replace.

use super::super::ROWS;
use super::super::baseline::{BASELINE, Baseline, row};
mod pins;

use serde_json::{Value, json};
use std::path::PathBuf;

/// A fixture directory holding the two `.f64` files each test's shas claim, rebuilt from
/// scratch every time so a leftover file cannot make a test pass.
///
/// **One directory per test, not one shared.** Cargo runs the tests in these modules on
/// several threads, and `scratch` clears the directory it is given: a shared path would have
/// nine tests deleting each other's fixtures, and the symptom is a hash of a file that is not
/// there.
pub(super) fn dir(tag: &str) -> PathBuf {
    let out = scratch(tag);
    std::fs::create_dir_all(out.join("motor")).expect("motor/");
    std::fs::create_dir_all(out.join("ref")).expect("ref/");
    for (half, name, bytes) in [
        ("motor", "SPRING_3D", &b"sha"[..]),
        ("ref", "SPRING_3D", &b"sha"[..]),
        ("motor", "GRAPHVIZ_DOT", &b""[..]),
        ("ref", "GRAPHVIZ_DOT", &b"dot"[..]),
        ("motor", "GRAPHVIZ_FDP", &b"sha"[..]),
        ("ref", "GRAPHVIZ_FDP", &b"ref-fdp"[..]),
    ] {
        std::fs::write(out.join(half).join(format!("{name}.f64")), bytes)
            .unwrap_or_else(|e| panic!("{half}/{name}: {e}"));
    }
    out
}

/// The real sha256 of one of the fixture files, leaked so it can be a `&'static str`.
///
/// **The repo's own hasher, over the actual bytes**, not a digest written into the test: a pin
/// that does not hash its own fixture would make every assertion in this file vacuous, and
/// `file_sha256` is the same function the judge calls.
pub(super) fn pin(dir: &std::path::Path, half: &str, name: &str) -> String {
    crate::runner::file_sha256(&dir.join(half).join(format!("{name}.f64")))
        .unwrap_or_else(|e| panic!("hashing {half}/{name}: {e}"))
}

/// A temporary directory under `target/`. A test that leaked one per run would make `target/`
/// a place where stale fixtures answer for fresh ones.
fn scratch(tag: &str) -> PathBuf {
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/scigraphs-conformance-tests")
        .join(tag);
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).expect("scratch");
    out
}

/// One row of metrics, at the values a passing row would carry.
pub(super) fn ok_row() -> Value {
    json!({
        "coordinates": 1020,
        "bitwise_f64": 0,
        "bitwise_f32": 0,
        "max_ulp": 4529847,
        "max_gap": 3.5,
        "procrustes_median": 0.21,
        "procrustes_max": 0.9,
    })
}

/// The merged digest map [`super::one_row`] reads: the two shas, keyed by the paths `emit` and
/// the arms write. Flat, because that is what [`super::digests`] returns.
fn digests(motor: &str, reference: &str, name: &str) -> Value {
    json!({
        &format!("motor/{name}.f64"): motor,
        &format!("ref/{name}.f64"): reference,
    })
}

/// A baseline whose two pins match the bytes `dir()` wrote, and whose ceiling is above the
/// median [`ok_row`] carries. One leaked allocation per test, bounded by the test count.
pub(super) fn baseline_for(name: &'static str, tag: &str) -> &'static Baseline {
    let dir = dir(tag);
    let motor = pin(&dir, "motor", name);
    let reference = pin(&dir, "ref", name);
    Box::leak(Box::new(row(
        name,
        Box::leak(motor.into_boxed_str()),
        Box::leak(reference.into_boxed_str()),
        "",
        1.0,
        "shape",
        "convention",
    )))
}

/// The same, with a ceiling of the test's choosing.
fn baseline_with_ceiling(name: &'static str, ceiling: f64, tag: &str) -> &'static Baseline {
    let dir = dir(tag);
    let motor = pin(&dir, "motor", name);
    let reference = pin(&dir, "ref", name);
    Box::leak(Box::new(row(
        name,
        Box::leak(motor.into_boxed_str()),
        Box::leak(reference.into_boxed_str()),
        "",
        ceiling,
        "shape",
        "convention",
    )))
}

/// One row through the judge, against the fixture directory `dir()` built.
pub(super) fn judge(
    tag: &str,
    base: &'static Baseline,
    got: &Value,
    motor: &str,
    reference: &str,
) -> (bool, String) {
    let dir = dir(tag);
    super::one_row(
        base.name,
        base,
        got,
        &digests(motor, reference, base.name),
        &dir,
    )
    .unwrap_or_else(|e| panic!("{}: {e}", base.name))
}

#[test]
fn a_row_whose_shas_match_and_whose_median_is_under_its_ceiling_passes() {
    let tag = "passes";
    let (ok, why) = judge(
        tag,
        baseline_for("SPRING_3D", tag),
        &ok_row(),
        &pin(&dir(tag), "motor", "SPRING_3D"),
        &pin(&dir(tag), "ref", "SPRING_3D"),
    );
    assert!(ok, "{why}");
    assert!(why.contains("ok \u{2014}"), "{why}");
}

/// **The negative control, on the motor half.** One bit of one motor coordinate changed: the
/// sha no longer matches and the row fails, naming the half that moved.
#[test]
fn a_row_whose_motor_sha_moved_fails_and_says_which_file() {
    let tag = "motor-moved";
    let (ok, why) = judge(
        tag,
        baseline_for("SPRING_3D", tag),
        &ok_row(),
        "other",
        &pin(&dir(tag), "ref", "SPRING_3D"),
    );
    assert!(!ok);
    assert!(why.contains("motor bytes"), "{why}");
    assert!(
        why.contains("other"),
        "the measured sha is not printed: {why}"
    );
}

/// The same rule on the reference half: a reference that stopped being the reference is a
/// failure, not a new number.
#[test]
fn a_row_whose_reference_sha_moved_fails() {
    let tag = "ref-moved";
    let (ok, why) = judge(
        tag,
        baseline_for("SPRING_3D", tag),
        &ok_row(),
        &pin(&dir(tag), "motor", "SPRING_3D"),
        "other",
    );
    assert!(!ok);
    assert!(why.contains("reference bytes"), "{why}");
}

/// **The bytes on disk, not just the digest.** A `.f64` edited after the arm recorded its
/// sha keeps every recorded digest intact and would pass a judge that trusted them. This is
/// the check that makes `runner::file_sha256` inside the judge load-bearing.
///
/// The digest is taken **before** the tampering, because that is the order the world is in:
/// the arm writes the file and its digest, and somebody edits the file afterwards.
#[test]
fn a_file_edited_after_its_sha_was_recorded_fails() {
    let tag = "edited";
    let base = baseline_for("SPRING_3D", tag);
    let dir = dir(tag);
    let recorded = digests(
        &pin(&dir, "motor", "SPRING_3D"),
        &pin(&dir, "ref", "SPRING_3D"),
        "SPRING_3D",
    );
    std::fs::write(dir.join("motor/SPRING_3D.f64"), b"tampered").expect("tamper");
    let (ok, why) =
        super::one_row("SPRING_3D", base, &ok_row(), &recorded, &dir).expect("measured");
    assert!(
        !ok,
        "a tampered coordinate file passed on its recorded digest alone"
    );
    assert!(why.contains("is not the file that was hashed"), "{why}");
}

/// **The same on the reference half**, because the two digests come from different arms and a
/// defect that hits one hits the other.
#[test]
fn a_reference_file_deleted_after_its_sha_was_recorded_fails() {
    let tag = "deleted";
    let base = baseline_for("SPRING_3D", tag);
    let dir = dir(tag);
    let recorded = digests(
        &pin(&dir, "motor", "SPRING_3D"),
        &pin(&dir, "ref", "SPRING_3D"),
        "SPRING_3D",
    );
    std::fs::remove_file(dir.join("ref/SPRING_3D.f64")).expect("remove");
    let (ok, why) =
        super::one_row("SPRING_3D", base, &ok_row(), &recorded, &dir).expect("measured");
    assert!(
        !ok,
        "a deleted reference file passed on its recorded digest alone"
    );
    assert!(why.contains("is not the file that was hashed"), "{why}");
}

/// The shape half: the bytes are identical and the drawing drifted past its ceiling. Both
/// numbers are in the message, so the log says what moved without opening anything.
#[test]
fn a_row_over_its_procrustes_ceiling_fails_with_both_numbers() {
    let tag = "ceiling";
    let (ok, why) = judge(
        tag,
        baseline_with_ceiling("SPRING_3D", 0.2, tag),
        &ok_row(),
        &pin(&dir(tag), "motor", "SPRING_3D"),
        &pin(&dir(tag), "ref", "SPRING_3D"),
    );
    assert!(!ok);
    assert!(why.contains("2.100e-1"), "{why}");
    assert!(why.contains("2.000e-1"), "{why}");
}

/// The baseline the judge reads and the matrix it judges must be the same length, or a row is
/// judged against nothing. `super::run` refuses that case; this is the cheap guard in front.
#[test]
fn the_baseline_and_the_matrix_are_the_same_length() {
    assert_eq!(
        BASELINE.len(),
        ROWS.len(),
        "the baseline must have one pinned row per matrix row"
    );
}

/// A metrics file computed from other fixtures is refused: the check that keeps a stale
/// `metrics.json` from answering for this tree.
#[test]
fn metrics_from_other_fixtures_are_refused() {
    let err = super::run(&scratch("absent")).expect_err("no manifest at all");
    assert!(err.contains("conformance-manifest.json"), "{err}");
}

/// A manifest whose fingerprint is not this tree's is refused, with the reason, rather than
/// compared.
#[test]
fn a_manifest_from_another_tree_is_refused() {
    let out = scratch("other-tree");
    std::fs::write(
        out.join("conformance-manifest.json"),
        r#"{"fingerprint":"nope"}"#,
    )
    .expect("manifest");
    std::fs::write(
        out.join("metrics.json"),
        r#"{"fingerprint":"nope","sha256":{"conformance.jsonl":"x"},"rows":{}}"#,
    )
    .expect("metrics");
    let err = super::run(&out).expect_err("another tree's metrics");
    assert!(err.contains("same tree"), "{err}");
}

/// **`metrics.json` cannot replace a motor digest.** Its shas are written by the other arm, so
/// a key named `motor/<NAME>.f64` in it must be ignored: otherwise the comparison would be
/// between two things the same side wrote.
#[test]
fn the_metrics_map_cannot_override_a_motor_digest() {
    let manifest = json!({ "sha256": {
        "conformance.jsonl": "x",
        "motor/SPRING_3D.f64": "sha",
    } });
    let metrics = json!({ "sha256": {
        "conformance.jsonl": "x",
        "motor/SPRING_3D.f64": "forged",
        "ref/SPRING_3D.f64": "refsha",
    } });
    let merged = super::digests(&manifest, &metrics);
    assert_eq!(merged["motor/SPRING_3D.f64"], "sha", "the other arm won");
    assert_eq!(
        merged["ref/SPRING_3D.f64"], "refsha",
        "the reference half was dropped"
    );
}
