//! The Phase 6 force layouts end to end: both stages 4-way hashed, each with its own
//! negative control going red on only its stage, and the `stress` and `bench` commands
//! that measure them. Split out of `cli.rs` to stay under the house's 300-line limit.
//!
//! Needs `node`, the `wasm32-unknown-unknown` target and `harness/node_modules`, as
//! `cli.rs` does.

mod common;

use common::{KNOBS, stdout};
use std::process::Output;

fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-force-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(&gates_dir(), args, mutate)
}

fn record(name: &str) -> String {
    std::fs::read_to_string(gates_dir().join(format!("{name}.json"))).expect("recorded")
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
