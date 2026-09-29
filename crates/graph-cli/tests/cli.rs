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

const KNOBS: [&str; 6] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_SUGIYAMA_LAYER_SPACING",
    "GM_MUTATE_NODE_COUNT",
    "GM_MUTATE_FORCE_THETA",
    "GM_MUTATE_FA2_SCALING_RATIO",
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

/// The two force stages are 4-way compared like every other, so the wasm arm must carry
/// them: a stage the wasm module cannot export would silently drop out of the
/// comparison rather than fail it.
#[test]
fn the_force_stages_are_4_way_compiled_and_hashed() {
    let honest = graph_cli(&["hashgate", "--seeds", "2"], None);
    assert_eq!(honest.status.code(), Some(0), "{}", stdout(&honest));
    for stage in ["layout.force.barnes_hut", "layout.forceatlas2"] {
        assert!(
            stdout(&honest).contains(&format!("  {stage}: 4-way equal on 2/2 seeds")),
            "{stage} did not go 4-way equal: {}",
            stdout(&honest)
        );
    }
    let record = record("hashgate");
    for stage in ["layout.force.barnes_hut", "layout.forceatlas2"] {
        assert!(
            record.contains(&format!("\"{stage}\": 2")),
            "{stage} is missing from the record: {record}"
        );
    }
}

/// Each force layout's own negative control, end to end: the wasm arm runs the
/// compiled-in default and cannot see the variable, so a wired knob shows up as exactly
/// the cross-target divergence — and only on the stage the knob is filed under.
///
/// Two things about this test are not incidental, and both were found by running it
/// rather than by reading it:
///
/// - **The seed count.** `gate_node_count(seed)` is `2 + seed % 600`, so `--seeds 2`
///   gives models of 2 and 3 nodes — and at those sizes the quadtree never opens a
///   cell, so *any* theta yields the identical many-body force. The control passed
///   vacuously, which is the one failure mode a negative control must never have.
///   40 seeds reaches models of up to 41 nodes, where theta reaches the opening test.
/// - **Not every seed diverges, and must not.** Theta only changes the result once the
///   tree actually opens a cell, so the control is required to diverge on *at least
///   one* seed — the ledger's own bar, and the bar that makes it evidence — while
///   leaving the untouched stages equal on *every* seed.
#[test]
fn each_force_layouts_own_control_goes_red_on_only_its_stage() {
    const SEEDS: &str = "40";
    for (knob, value, stage, record_name) in [
        (
            KNOBS[4],
            "0.5",
            "layout.force.barnes_hut",
            "hashgate-control-force-theta",
        ),
        (
            KNOBS[5],
            "3",
            "layout.forceatlas2",
            "hashgate-control-fa2-scaling-ratio",
        ),
    ] {
        let run = graph_cli(&["hashgate", "--seeds", SEEDS], Some((knob, value)));
        assert_eq!(
            run.status.code(),
            Some(1),
            "{knob}={value}: {}",
            stdout(&run)
        );
        let out = stdout(&run);
        // At least one seed must diverge, or the control backs nothing.
        let line = out
            .lines()
            .find(|l| l.trim_start().starts_with(&format!("{stage}:")))
            .unwrap_or_else(|| panic!("{stage} is not reported: {out}"));
        let equal: usize = line
            .rsplit_once("on ")
            .and_then(|(_, rest)| rest.split('/').next())
            .and_then(|n| n.trim().parse().ok())
            .unwrap_or_else(|| panic!("no seed count in {line:?}"));
        assert!(equal < 40, "{knob} did not go red on {stage}: {line}");
        // Its own stage only: a control that moved anything else would back every
        // stage at once and prove nothing about the stage it is filed under.
        for other in [
            "topology",
            "layout.grid",
            "layout.tree.tidy",
            "layout.treemap.squarified",
            "layout.circular.radial",
            "layout.packing.circle",
        ] {
            assert!(
                out.contains(&format!("  {other}: 4-way equal on {SEEDS}/{SEEDS} seeds")),
                "{knob} must not move {other}: {out}"
            );
        }
        let control = record(record_name);
        assert!(control.contains("\"pass\": false"), "{control}");
        assert!(
            control.contains(&format!("\"mutation\": \"{knob}\"")),
            "{control}"
        );
    }
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
        "layout.force.barnes_hut",
        "layout.forceatlas2",
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

/// `stress --oracle d3` is the quality gate a force layout can actually be held to:
/// identity is impossible, so the claim is "different, but not worse" — our stress
/// correlation must clear d3-force@3.0.0's own by the frozen -0.05 margin. It needs
/// `d3-force` resolvable, which is why this test treats a missing module as
/// "could not run" (exit 2), never as a pass.
#[test]
fn stress_needs_an_oracle_and_runs_the_d3_arm_or_says_it_could_not() {
    assert_eq!(graph_cli(&["stress"], None).status.code(), Some(2));
    let run = graph_cli(&["stress", "--oracle", "d3", "--seeds", "2"], None);
    let out = stdout(&run);
    match run.status.code() {
        Some(0) => {
            assert!(out.contains("stress"), "{out}");
            // A passing run must state the margin it checked, or "not worse" is unfalsifiable.
            assert!(out.contains("-0.05") || out.contains("margin"), "{out}");
            let record = record("stress");
            assert!(record.contains("\"pass\": true"), "{record}");
            assert!(record.contains("layout.force.barnes_hut"), "{record}");
        }
        Some(2) => assert!(
            out.contains("d3-force") || String::from_utf8_lossy(&run.stderr).contains("d3-force"),
            "exit 2 must name the missing module: {out}"
        ),
        other => {
            panic!("stress must pass (0) or report it could not run (2), got {other:?}: {out}")
        }
    }
}

/// `bench` times the force layouts at the sizes the phase gate names, and refuses a
/// size past a layout's own registered ceiling rather than running for minutes: the
/// ceiling is a budget, and a budget a command silently blows through is not a gate.
///
/// The refusal is exit **0**, not 1: the phase gate's own size set
/// (`220,10000,100000`) includes 100 000, which FA2 refuses, so a non-zero code there
/// would make the gate row unsatisfiable. The refusal is reported on stdout instead,
/// and `--dry-run` exercises that decision without paying for it — which is what makes
/// it testable at all, since Barnes-Hut at 15 000 nodes takes about 25 s.
#[test]
fn bench_times_the_force_layouts_and_refuses_a_size_past_a_ceiling() {
    let run = graph_cli(&["bench", "--n", "40,60"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stdout(&run));
    let out = stdout(&run);
    assert!(out.contains("layout.force.barnes_hut"), "{out}");
    assert!(out.contains("layout.forceatlas2"), "{out}");
    assert!(out.contains("n=40") && out.contains("n=60"), "{out}");
    assert!(out.contains("ms"), "a timing must be printed: {out}");
    let refused = graph_cli(&["bench", "--n", "15000", "--dry-run"], None);
    let text = stdout(&refused);
    assert_eq!(refused.status.code(), Some(0), "{text}");
    assert!(
        text.contains("14000") && text.contains("layout.forceatlas2"),
        "{text}"
    );
    assert!(
        text.contains("would run") && text.contains("layout.force.barnes_hut"),
        "the same run must say Barnes-Hut would still have run: {text}"
    );
    for bad in ["0", "-1", "abc", ""] {
        assert_eq!(
            graph_cli(&["bench", "--n", bad], None).status.code(),
            Some(2),
            "--n {bad:?} must not run"
        );
    }
}

#[test]
fn hashgate_arm_prints_one_line_per_stage_and_seed() {
    let arm = graph_cli(&["hashgate-arm", "--seeds", "3"], None);
    assert_eq!(arm.status.code(), Some(0));
    let lines: Vec<String> = stdout(&arm).lines().map(str::to_owned).collect();
    assert_eq!(lines.len(), 33, "11 stages * 3 seeds");
    assert!(lines[2].starts_with("topology 2 ") && lines[2].len() == "topology 2 ".len() + 64);
    let grid = "layout.grid 2 ";
    assert!(lines[5].starts_with(grid) && lines[5].len() == grid.len() + 64);
    let last = "layout.dag.sugiyama 2 ";
    assert!(lines[32].starts_with(last) && lines[32].len() == last.len() + 64);
}

#[test]
fn capabilities_needs_a_flag_and_refuses_gated_rows_no_recorded_run_backs() {
    assert_eq!(graph_cli(&["capabilities"], None).status.code(), Some(2));
    let check = graph_cli(&["capabilities", "--check"], None);
    assert_eq!(check.status.code(), Some(1), "{}", stdout(&check));
    assert!(stdout(&check).contains("capabilities --check: 18 rows, 32 problems"));
    let json = graph_cli(&["capabilities", "--json"], None);
    assert_eq!(json.status.code(), Some(0));
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    assert_eq!(rows.as_array().map(Vec::len), Some(18));
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
