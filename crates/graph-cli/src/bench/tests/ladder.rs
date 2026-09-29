//! The crossover table: a `Sample`, an arm reading, and the arithmetic that turns them
//! into markdown a reader can compare across arms.

use super::super::campaign::Sample;
use super::super::campaign::arms::{self, ArmReading};

#[test]
fn a_sample_reports_a_one_shot_run_and_the_tick_it_implies() {
    let sample = Sample {
        n: 220,
        edges: 327,
        run_ms: 1120.0,
        build_ms: 1.0,
        columns: 0,
        arena: 0,
        bin: 0,
        json: 0,
        past_ceiling: false,
    };
    // The ABI has no per-tick entry, so the campaign derives it: the same division the
    // two harness arms make, which is what lets three crossover cells be compared.
    assert_eq!(sample.tick_ms(), 10.0);
    assert_eq!(sample.settle_ms(), 1120.0);
}

#[test]
fn a_measured_arm_fills_its_crossover_cell_and_an_absent_one_says_so() {
    let measured = ArmReading::measured("wasm32", Some(220), &[(220, 0.319), (10_000, 34.97)]);
    let absent = ArmReading::absent(
        "TypeScript oracle",
        "harness/oracle-tick-bench.mjs was not run",
    );
    let text = arms::arms_markdown(&[measured, absent], 16.67);
    assert!(text.contains("| wasm32 | 220 |"), "{text}");
    assert!(text.contains("not measured"), "{text}");
    assert!(
        text.contains("harness/oracle-tick-bench.mjs was not run"),
        "{text}"
    );
}

/// The harness JSON the two JS arms write is read into the crossover table, and a
/// truncated or foreign file is a refusal naming it, never a silent zero.
#[test]
fn an_arm_json_file_is_read_into_its_ladder_and_a_bad_one_is_refused() {
    let dir = std::env::temp_dir().join(format!("gm-arms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("scratch");
    let path = dir.join("arm.json");
    let good = r#"{"arm":"wasm32","rows":[{"n":220,"tick_ms":0.319},{"n":10000,"tick_ms":34.97}]}"#;
    std::fs::write(&path, good).expect("written");
    let rows = arms::read_arm_json(&path).expect("the good file reads");
    assert_eq!(rows, vec![(220, 0.319), (10_000, 34.97)]);
    std::fs::write(&path, "{ not json").expect("written");
    let err = arms::read_arm_json(&path).expect_err("a truncated file is refused");
    assert!(err.contains("arm.json"), "{err}");
    std::fs::remove_dir_all(&dir).ok();
}
