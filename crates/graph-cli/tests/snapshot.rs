//! `snapshot` and `roundtrip`: the two consumer-facing subcommands whose output a
//! script reads directly. Split out of `cli.rs` to keep both files under the house line
//! limit; the small process helpers below are duplicated from `cli.rs` rather than
//! shared, so each file stays self-contained.
//!
//! Needs `node` and the `wasm32-unknown-unknown` target, as `cli.rs` does.

use std::process::{Command, Output};

/// Gate records land here, never in `target/gates`: a test run must not overwrite (or
/// stand in for) the evidence of a real gate run.
fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-gates-{}", std::process::id()))
}

const KNOBS: [&str; 2] = ["GM_MUTATE_REFERENCE_DEGREE", "GM_MUTATE_GRID_SPACING"];

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

/// The consumer's command emitting both faces, one to standard output.
#[test]
fn snapshot_emits_either_face_on_success() {
    let bin = std::env::temp_dir().join(format!("gm-cli-snapshot-{}.bin", std::process::id()));
    let path = bin.to_str().expect("utf-8");
    let args = [
        "snapshot", "--seed", "1", "--nodes", "50", "--layout", "grid",
    ];
    let run = graph_cli(
        &[&args[..], &["--out-bin", path, "--out-json", "-"]].concat(),
        None,
    );
    assert_eq!(
        run.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let json = stdout(&run);
    assert!(
        json.starts_with("{\"edges\":{\"id\":[") && json.ends_with("}\n"),
        "{json}"
    );
    assert!(
        json.contains(
            "\"notes\":{\"code\":[],\"index\":[]},\"version\":{\"major\":0,\"minor\":3}}"
        ),
        "{json}"
    );
    let summary = String::from_utf8_lossy(&run.stderr).into_owned();
    assert!(
        summary.starts_with("snapshot: seed 1, layout.grid, 50 nodes, "),
        "{summary}"
    );
    let bytes = std::fs::read(&bin).expect("written");
    assert_eq!(&bytes[..4], b"GMSN");
    std::fs::remove_file(&bin).expect("cleanup");
}

/// Every refusal the consumer's command can hit, each with the exit code a script can
/// branch on.
const REFUSALS: [(&[&str], i32); 4] = [
    (&["snapshot", "--seed", "1", "--layout", "grid"], 2),
    (
        &[
            "snapshot",
            "--seed",
            "1",
            "--layout",
            "spiral",
            "--out-json",
            "-",
        ],
        2,
    ),
    (
        &[
            "snapshot",
            "--seed",
            "1",
            "--nodes",
            "0",
            "--layout",
            "grid",
            "--out-json",
            "-",
        ],
        2,
    ),
    (
        &[
            "snapshot",
            "--seed",
            "1",
            "--layout",
            "grid",
            "--out-bin",
            "-",
            "--out-json",
            "-",
        ],
        2,
    ),
];

#[test]
fn snapshot_refuses_what_it_cannot_do() {
    for (args, code) in REFUSALS {
        let run = graph_cli(args, None);
        assert_eq!(run.status.code(), Some(code), "{args:?}");
        assert!(run.stdout.is_empty(), "{args:?} wrote to stdout");
    }
}

#[test]
fn roundtrip_passes_and_records_the_grids_hand_oracle() {
    let run = graph_cli(&["roundtrip", "--seeds", "20"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stdout(&run));
    assert!(stdout(&run).contains("  binary <-> JSON byte-exact on 140/140 snapshots"));
    assert!(stdout(&run).contains("  layout.grid on its stated conventions on 20/20 seeds"));
    assert!(
        stdout(&run).contains("  layout.circular.radial on its stated conventions on 20/20 seeds")
    );
    assert!(
        stdout(&run).contains("  layout.packing.circle on its stated conventions on 20/20 seeds")
    );
    assert!(
        stdout(&run).contains("  layout.dag.sugiyama on its structural invariants on 20/20 seeds")
    );
    assert!(stdout(&run).contains(
        "  notes cases drawn (exercise, each needed): 0.2-labelled 4, 0.3 k=0 5, code 1 "
    ));
    assert!(stdout(&run).ends_with("PASS\n"));
    let roundtrip = record("roundtrip");
    assert!(
        roundtrip.contains("\"pass\": true") && roundtrip.contains("\"cases\": 20"),
        "{roundtrip}"
    );
    assert!(
        roundtrip.contains("\"layout.circular.radial\"")
            && roundtrip.contains("\"layout.packing.circle\"")
            && roundtrip.contains("\"layout.dag.sugiyama\""),
        "{roundtrip}"
    );
    assert_eq!(
        graph_cli(&["roundtrip", "--seeds", "0"], None)
            .status
            .code(),
        Some(2)
    );
}
