//! `snapshot` and `roundtrip`: the two consumer-facing subcommands whose output a
//! script reads directly. Split out of `cli.rs` to keep both files under the house line
//! limit; the process helpers are `tests/common`'s.
//!
//! Needs `node` and the `wasm32-unknown-unknown` target, as `cli.rs` does.

mod common;

use common::stdout;
use std::process::Output;

/// Gate records land here, never in `target/gates`: a test run must not overwrite (or
/// stand in for) the evidence of a real gate run.
fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-gates-{}", std::process::id()))
}

fn graph_cli(args: &[&str], mutate: Option<(&str, &str)>) -> Output {
    common::graph_cli(&gates_dir(), args, mutate)
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
            "helix",
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

/// 20 seeds over every registered layout plus the contract exercise, counted from the
/// ledger the binary publishes rather than written into the test.
fn roundtrip_total(seeds: u32) -> String {
    let json = graph_cli(&["capabilities", "--json"], None);
    assert_eq!(json.status.code(), Some(0));
    let rows: serde_json::Value = serde_json::from_str(&stdout(&json)).expect("json");
    let layouts = rows
        .as_array()
        .expect("an array")
        .iter()
        .filter(|r| r["stage"] == "layout")
        .count();
    (seeds as usize * (layouts + 1)).to_string()
}

#[test]
fn roundtrip_passes_and_records_the_grids_hand_oracle() {
    let expected = roundtrip_total(20);
    let run = graph_cli(&["roundtrip", "--seeds", "20"], None);
    assert_eq!(run.status.code(), Some(0), "{}", stdout(&run));
    // Every registered layout plus the contract exercise, per seed. The count is read
    // from the ledger rather than written here: it moves with the registry, and a literal
    // would pin the stage list instead of the round trip's own arithmetic.
    assert!(
        stdout(&run).contains(&format!(
            "  binary <-> JSON byte-exact on {}/{expected} snapshots",
            expected
        )),
        "{}",
        stdout(&run)
    );
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
    // A 3D snapshot cannot be 0.2-labelled (0.2 names no dimension), so the exercise's
    // 3D seeds draw the 0.3 "no notes" case instead and the 0.2 tally is one lower than
    // it was with every seed 2D. Every case is still drawn, which is what `pass` requires.
    assert!(stdout(&run).contains(
        "  notes cases drawn (exercise, each needed): 0.2-labelled 3, 0.3 k=0 6, code 1 6, code 2 7, code 3 4"
    ));
    // The 3D half of the sweep, counted rather than assumed: `seed % 3 == 2` draws a third
    // of the 20 exercise snapshots, which is 6 (20 seeds, 2 and 5 fall in the 0.2 slot).
    assert!(
        stdout(&run).contains("  3D exercise snapshots (dim 1, z column) round-tripped: 6"),
        "{}",
        stdout(&run)
    );
    assert!(stdout(&run).ends_with("PASS\n"));
    let roundtrip = record("roundtrip");
    assert!(
        roundtrip.contains("\"pass\": true") && roundtrip.contains("\"cases\": 20"),
        "{roundtrip}"
    );
    assert!(
        roundtrip.contains("\"three_d_exercise\": 6"),
        "the ledger records the 3D count too, not only the text: {roundtrip}"
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
