//! What `run` reads before it judges anything: the manifest and the metrics file have to be
//! this tree's, the matrix and the baseline have to be the same table, and neither arm's
//! own file may be able to overwrite the other's digest.

use super::super::{ROWS, digests, run};
use super::harness::scratch;
use crate::oracle_python::conformance::baseline::BASELINE;
use serde_json::json;

/// The baseline the judge reads and the matrix it judges must be the same length, or a row is
/// judged against nothing. `run` refuses that case; this is the cheap guard in front.
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
    let err = run(&scratch("absent")).expect_err("no manifest at all");
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
    let err = run(&out).expect_err("another tree's metrics");
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
    let merged = digests(&manifest, &metrics);
    assert_eq!(merged["motor/SPRING_3D.f64"], "sha", "the other arm won");
    assert_eq!(
        merged["ref/SPRING_3D.f64"], "refsha",
        "the reference half was dropped"
    );
}
