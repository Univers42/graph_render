//! The fixture directory, the pins and the one-call judge every test in this directory runs
//! against: the harness the other three files share.

use crate::oracle_python::conformance::baseline::{Baseline, row};
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
pub(super) fn scratch(tag: &str) -> PathBuf {
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

/// The merged digest map `one_row` reads: the two shas, keyed by the paths `emit` and
/// the arms write. Flat, because that is what `judge` hands it as.
pub(super) fn digests(motor: &str, reference: &str, name: &str) -> Value {
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
pub(super) fn baseline_with_ceiling(
    name: &'static str,
    ceiling: f64,
    tag: &str,
) -> &'static Baseline {
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
    super::super::one_row(
        base.name,
        base,
        got,
        &digests(motor, reference, base.name),
        &dir,
    )
    .unwrap_or_else(|e| panic!("{}: {e}", base.name))
}
