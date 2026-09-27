//! The gates, run the way a phase gate runs them: as the real `graph-cli` binary, checked
//! by exit code. The negative control lives here too, so a gate that stops going red
//! fails `cargo test`, not only a human's reading of a log.
//!
//! Needs `node` and the `wasm32-unknown-unknown` target, as the gates do; both are in
//! the `ge-rust` image.

use std::process::{Command, Output};

/// Gate records land here, never in `target/gates`: a test run must not overwrite (or
/// stand in for) the evidence of a real gate run.
fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
    command.args(args).env_remove("GM_MUTATE_REFERENCE_DEGREE");
    command.env("GM_GATES_DIR", gates_dir());
    if let Some(value) = mutate {
        command.env("GM_MUTATE_REFERENCE_DEGREE", value);
    }
    command.output().expect("graph-cli runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn hashgate_passes_and_its_negative_control_goes_red() {
    let honest = graph_cli(&["hashgate", "--seeds", "4"], None);
    assert_eq!(honest.status.code(), Some(0), "{}", stdout(&honest));
    assert!(stdout(&honest).contains("  topology: 4-way equal on 4/4 seeds"));
    assert!(stdout(&honest).contains("  4-way equal on 4/4 seeds"));
    let record = std::fs::read_to_string(gates_dir().join("hashgate.json")).expect("recorded");
    assert!(
        record.contains("\"pass\": true") && record.contains("\"topology\": 4"),
        "{record}"
    );

    let mutated = graph_cli(&["hashgate", "--seeds", "4"], Some("9"));
    assert_eq!(mutated.status.code(), Some(1), "{}", stdout(&mutated));
    assert!(stdout(&mutated).contains("  topology: 4-way equal on 0/4 seeds"));
    assert!(stdout(&mutated).contains("  4-way equal on 0/4 seeds"));
    let control = gates_dir().join("hashgate-control.json");
    let control = std::fs::read_to_string(control).expect("control recorded");
    assert!(control.contains("\"pass\": false"), "{control}");
    assert!(stdout(&mutated).contains("FAIL: 4 of 4 seeds diverge"));

    let typo = graph_cli(&["hashgate", "--seeds", "4"], Some("nine"));
    assert_eq!(typo.status.code(), Some(2), "a typo must not pass as green");
}

#[test]
fn a_failed_wasm_build_is_could_not_run_and_seed_counts_are_capped() {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
    let broken = command
        .args(["hashgate", "--seeds", "2"])
        .env("CARGO", "false")
        .output()
        .expect("graph-cli runs");
    assert_eq!(broken.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(
        stderr.contains("building graph-wasm for wasm32 failed"),
        "{stderr}"
    );
    assert_eq!(
        graph_cli(&["hashgate", "--seeds", "100001"], None)
            .status
            .code(),
        Some(2)
    );
}

#[test]
fn codegen_check_finds_the_committed_files_current() {
    let check = graph_cli(&["codegen", "--check"], None);
    assert_eq!(check.status.code(), Some(0), "{}", stdout(&check));
    assert_eq!(stdout(&check).matches("up to date").count(), 2);
}

#[test]
fn hashgate_arm_prints_one_line_per_stage_and_seed() {
    let arm = graph_cli(&["hashgate-arm", "--seeds", "3"], None);
    assert_eq!(arm.status.code(), Some(0));
    let lines: Vec<String> = stdout(&arm).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 6);
    assert!(lines[2].starts_with("synthetic 2 ") && lines[2].len() == "synthetic 2 ".len() + 64);
    assert!(lines[5].starts_with("topology 2 ") && lines[5].len() == "topology 2 ".len() + 64);
}

#[test]
fn capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs() {
    assert_eq!(graph_cli(&["capabilities"], None).status.code(), Some(2));
    let check = graph_cli(&["capabilities", "--check"], None);
    assert_eq!(check.status.code(), Some(1), "{}", stdout(&check));
    assert!(stdout(&check).contains("capabilities --check: 8 rows, 16 problems"));
    let json = graph_cli(&["capabilities", "--json"], None);
    assert_eq!(json.status.code(), Some(0));
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    assert_eq!(rows.as_array().map(Vec::len), Some(8));
    assert!(
        rows[0]["oracle_diff"]
            .as_str()
            .is_some_and(|s| s.starts_with("not backed: "))
    );
}

#[test]
fn determinism_probe_writes_a_measurement_with_libm_agreeing_across_targets() {
    let out = std::env::temp_dir().join(format!("gm-d1-{}.md", std::process::id()));
    let run = graph_cli(
        &["determinism-probe", "--out", out.to_str().expect("utf-8")],
        None,
    );
    assert_eq!(run.status.code(), Some(0), "{}", stdout(&run));
    let doc = std::fs::read_to_string(&out).expect("measurement written");
    std::fs::remove_file(&out).expect("temp file removable");
    assert!(doc.starts_with("# D1") && doc.contains("Toolchain: rustc "));
    assert!(doc.contains("| `ln_1p` | 2088 | "));
    assert!(doc.contains("is bit-identical between native and wasm32"));
    assert!(stdout(&run).contains(&format!("wrote {}", out.display())));
}
