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

const KNOBS: [&str; 4] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
    "GM_MUTATE_NODE_COUNT",
];

/// `graph-cli args` with every knob unset but `mutate`, if given.
fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
    command.args(args).env("GM_GATES_DIR", gates_dir());
    for knob in KNOBS {
        command.env_remove(knob);
    }
    if let Some((knob, value)) = mutate {
        command.env(knob, value);
    }
    command.output().expect("graph-cli runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn record(name: &str) -> String {
    std::fs::read_to_string(gates_dir().join(format!("{name}.json"))).expect("recorded")
}

#[test]
fn hashgate_passes_on_an_honest_run() {
    let honest = graph_cli(&["hashgate", "--seeds", "4"], None);
    assert_eq!(honest.status.code(), Some(0), "{}", stdout(&honest));
    assert!(stdout(&honest).contains("  topology: 4-way equal on 4/4 seeds"));
    assert!(stdout(&honest).contains("  layout.grid: 4-way equal on 4/4 seeds"));
    assert!(stdout(&honest).contains("  layout.dag.sugiyama: 4-way equal on 4/4 seeds"));
    assert!(stdout(&honest).contains("  4-way equal on 4/4 seeds"));
    assert!(stdout(&honest).contains("  native run 1  digest "));
    assert!(!stdout(&honest).contains("DIVERGED"));
    let honest = record("hashgate");
    assert!(
        honest.contains("\"pass\": true") && honest.contains("\"layout.grid\": 4"),
        "{honest}"
    );
}

#[test]
fn each_negative_control_goes_red_on_its_own_stage() {
    let degree = graph_cli(&["hashgate", "--seeds", "4"], Some((KNOBS[0], "9")));
    assert_eq!(degree.status.code(), Some(1), "{}", stdout(&degree));
    assert!(stdout(&degree).contains("  topology: 4-way equal on 0/4 seeds"));
    assert!(stdout(&degree).contains("  layout.grid: 4-way equal on 4/4 seeds"));
    assert!(stdout(&degree).contains("  DIVERGED topology 0:"));
    assert!(stdout(&degree).contains("FAIL: 4 of 4 seeds diverge"));
    let control = record("hashgate-control-reference-degree");
    assert!(control.contains("\"pass\": false"), "{control}");
    assert!(
        control.contains("\"mutation\": \"GM_MUTATE_REFERENCE_DEGREE\""),
        "{control}"
    );

    let spacing = graph_cli(&["hashgate", "--seeds", "4"], Some((KNOBS[1], "2")));
    assert_eq!(spacing.status.code(), Some(1), "{}", stdout(&spacing));
    assert!(stdout(&spacing).contains("  topology: 4-way equal on 4/4 seeds"));
    assert!(stdout(&spacing).contains("  layout.grid: 4-way equal on 0/4 seeds"));
    assert!(stdout(&spacing).contains("  DIVERGED layout.grid 0:"));
    let control = record("hashgate-control-grid-spacing");
    assert!(control.contains("\"pass\": false"), "{control}");
    assert!(stdout(&spacing).contains("  layout.dag.sugiyama: 4-way equal on 4/4 seeds"));

    let layers = graph_cli(&["hashgate", "--seeds", "4"], Some((KNOBS[2], "2")));
    assert_eq!(layers.status.code(), Some(1), "{}", stdout(&layers));
    assert!(stdout(&layers).contains("  topology: 4-way equal on 4/4 seeds"));
    assert!(stdout(&layers).contains("  layout.grid: 4-way equal on 4/4 seeds"));
    assert!(stdout(&layers).contains("  layout.dag.sugiyama: 4-way equal on 0/4 seeds"));
    assert!(stdout(&layers).contains("  DIVERGED layout.dag.sugiyama 0:"));
    let control = record("hashgate-control-sugiyama-layer-spacing");
    assert!(control.contains("\"pass\": false"), "{control}");

    for (knob, typo) in [
        (KNOBS[0], "nine"),
        (KNOBS[1], "wide"),
        (KNOBS[1], "0"),
        (KNOBS[2], "tall"),
        (KNOBS[2], "0"),
    ] {
        let run = graph_cli(&["hashgate", "--seeds", "4"], Some((knob, typo)));
        assert_eq!(
            run.status.code(),
            Some(2),
            "{knob}={typo} must not pass as a control"
        );
    }
    let both = Command::new(env!("CARGO_BIN_EXE_graph-cli"))
        .args(["hashgate", "--seeds", "4"])
        .env("GM_GATES_DIR", gates_dir())
        .env(KNOBS[0], "9")
        .env(KNOBS[1], "2")
        .output()
        .expect("runs");
    assert_eq!(both.status.code(), Some(2), "one control at a time");
}

/// Tidy tree, circular and packing take no parameters, so nothing but the model itself
/// can move them natively; [`GM_MUTATE_NODE_COUNT`] backs their stages (and, honestly,
/// topology's and the others' too, since one more node moves every stage that is a
/// function of the topology at all).
#[test]
fn the_node_count_control_goes_red_on_every_stage_it_touches() {
    let grown = graph_cli(&["hashgate", "--seeds", "4"], Some((KNOBS[3], "1")));
    assert_eq!(grown.status.code(), Some(1), "{}", stdout(&grown));
    for stage in [
        "topology",
        "layout.grid",
        "layout.tree.tidy",
        "layout.treemap.squarified",
        "layout.circular.radial",
        "layout.packing.circle",
        "layout.spectral",
        "layout.mds.pivot",
        "layout.dag.sugiyama",
    ] {
        assert!(
            stdout(&grown).contains(&format!("  {stage}: 4-way equal on 0/4 seeds")),
            "{stage} did not diverge: {}",
            stdout(&grown)
        );
    }
    let control = record("hashgate-control-node-count");
    assert!(control.contains("\"pass\": false"), "{control}");
    assert!(
        control.contains("\"mutation\": \"GM_MUTATE_NODE_COUNT\""),
        "{control}"
    );
    let bad = graph_cli(&["hashgate", "--seeds", "4"], Some((KNOBS[3], "-1")));
    assert_eq!(
        bad.status.code(),
        Some(2),
        "signed input must not pass as a control"
    );
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
    assert_eq!(stdout(&check).matches("up to date").count(), 3);
}

#[test]
fn hashgate_arm_prints_one_line_per_stage_and_seed() {
    let arm = graph_cli(&["hashgate-arm", "--seeds", "3"], None);
    assert_eq!(arm.status.code(), Some(0));
    let lines: Vec<String> = stdout(&arm).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 27, "9 stages * 3 seeds");
    assert!(lines[2].starts_with("topology 2 ") && lines[2].len() == "topology 2 ".len() + 64);
    let grid = "layout.grid 2 ";
    assert!(lines[5].starts_with(grid) && lines[5].len() == grid.len() + 64);
    let last = "layout.dag.sugiyama 2 ";
    assert!(lines[26].starts_with(last) && lines[26].len() == last.len() + 64);
}

#[test]
fn capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs() {
    assert_eq!(graph_cli(&["capabilities"], None).status.code(), Some(2));
    let check = graph_cli(&["capabilities", "--check"], None);
    assert_eq!(check.status.code(), Some(1), "{}", stdout(&check));
    assert!(stdout(&check).contains("capabilities --check: 16 rows, 32 problems"));
    let json = graph_cli(&["capabilities", "--json"], None);
    assert_eq!(json.status.code(), Some(0));
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    assert_eq!(rows.as_array().map(Vec::len), Some(16));
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
        command.args(args).env("GM_GATES_DIR", &dir);
        for knob in KNOBS {
            command.env_remove(knob);
        }
        command.output().expect("graph-cli runs")
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

// The oracle differentials (`oracle-diff`, `oracle-layouts`) and their own negative
// controls live in `cli_oracles.rs`, split out to stay under the house's 300-line limit.

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
