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
    assert!(stdout(&honest).contains("  native run 1  digest "));
    assert!(!stdout(&honest).contains("DIVERGED"));
    let record = std::fs::read_to_string(gates_dir().join("hashgate.json")).expect("recorded");
    assert!(
        record.contains("\"pass\": true") && record.contains("\"topology\": 4"),
        "{record}"
    );

    let mutated = graph_cli(&["hashgate", "--seeds", "4"], Some("9"));
    assert_eq!(mutated.status.code(), Some(1), "{}", stdout(&mutated));
    assert!(stdout(&mutated).contains("  topology: 4-way equal on 0/4 seeds"));
    assert!(stdout(&mutated).contains("  4-way equal on 0/4 seeds"));
    assert!(stdout(&mutated).contains("  DIVERGED synthetic 0:"));
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

/// The ledger reads what a gate recorded: a short honest run is found, and refused for
/// its seed count rather than reported missing.
#[test]
fn the_ledger_reads_a_recorded_run_and_names_what_it_lacks() {
    let dir = std::env::temp_dir().join(format!("gm-cli-ledger-{}", std::process::id()));
    let run = |args: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
        let command = command.args(args).env("GM_GATES_DIR", &dir);
        command
            .env_remove("GM_MUTATE_REFERENCE_DEGREE")
            .output()
            .expect("graph-cli runs")
    };
    assert_eq!(run(&["hashgate", "--seeds", "2"]).status.code(), Some(0));
    let json = run(&["capabilities", "--json"]);
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    assert_eq!(
        rows[0]["hash_4way"],
        "not backed: hashgate ran 2 seeds, need 1000"
    );
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// The oracle differential end to end: emit, run the TypeScript arm, then feed it a
/// graph-core line one byte off (manifest digest fixed up, as a real bug would leave it)
/// and expect red — the harness's negative control, inside `cargo test`.
#[test]
fn oracle_diff_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_line() {
    let dir = std::env::temp_dir().join(format!("gm-cli-fixtures-{}", std::process::id()));
    let out = dir.to_str().expect("utf-8");
    let emit = graph_cli(&["emit-fixtures", "--seeds", "3", "--out", out], None);
    assert_eq!(emit.status.code(), Some(0), "{}", stdout(&emit));
    let diff = graph_cli(&["oracle-diff", "--fixtures", out], None);
    assert_eq!(diff.status.code(), Some(0), "{}", stdout(&diff));
    assert!(stdout(&diff).contains("0 unexplained") && stdout(&diff).ends_with("PASS\n"));
    let record = std::fs::read_to_string(gates_dir().join("oracle-diff.json")).expect("recorded");
    assert!(record.contains("\"pass\": true"), "{record}");

    let text = std::fs::read_to_string(dir.join("expect.jsonl")).expect("expect.jsonl");
    let red = diff_corrupted(&dir, &text, ("\"notes\":0}", "\"notes\":1}"));
    assert_eq!(red.status.code(), Some(1), "{}", stdout(&red));
    assert!(stdout(&red).contains("MISMATCH") && stdout(&red).contains("FAIL: 1 unexplained"));
    // H9's rule accepts graph-core's group only where it is the untruncated index: a
    // group off by 256 agrees with the oracle's byte and must still be red.
    let off = diff_corrupted(&dir, &text, ("\n[0,0,0,0,0,0]\n", "\n[256,0,0,0,0,0]\n"));
    assert_eq!(off.status.code(), Some(1), "{}", stdout(&off));
    assert!(stdout(&off).contains("MISMATCH line") && stdout(&off).contains("layoutGroups"));

    std::fs::remove_dir_all(&dir).expect("cleanup");
    let missing = graph_cli(&["oracle-diff", "--fixtures", out], None);
    assert_eq!(
        missing.status.code(),
        Some(2),
        "no fixtures is could-not-run"
    );
}

/// Runs the harness on the fixtures in `dir` with `expect.jsonl` set to `text` with one
/// line corrupted, and the manifest's digest updated to match, so only the harness's
/// comparison can catch it.
fn diff_corrupted(dir: &std::path::Path, text: &str, (from, to): (&str, &str)) -> Output {
    let (expect, manifest) = (dir.join("expect.jsonl"), dir.join("manifest.json"));
    let wrong = text.replacen(from, to, 1);
    assert_ne!(wrong, text, "a line holding {from:?} to corrupt");
    let before = std::fs::read(&expect).expect("expect.jsonl");
    std::fs::write(&expect, &wrong).expect("write");
    let fixed = std::fs::read_to_string(&manifest)
        .expect("manifest")
        .replace(&sha256_hex(&before), &sha256_hex(wrong.as_bytes()));
    std::fs::write(&manifest, fixed).expect("write");
    graph_cli(
        &["oracle-diff", "--fixtures", dir.to_str().expect("utf-8")],
        None,
    )
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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
