//! One row through the judge, and what each of the four ways it can be wrong has to say:
//! a motor sha that moved, a reference sha that moved, a file edited or deleted after its
//! digest was recorded, and a drawing past its ceiling.

use super::super::one_row;
use super::harness::{baseline_for, baseline_with_ceiling, digests, dir, judge, ok_row, pin};

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
    let (ok, why) = one_row("SPRING_3D", base, &ok_row(), &recorded, &dir).expect("measured");
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
    let (ok, why) = one_row("SPRING_3D", base, &ok_row(), &recorded, &dir).expect("measured");
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
