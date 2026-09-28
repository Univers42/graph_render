//! The two oracle differentials end to end, each with its own negative control inside
//! `cargo test`: `oracle-diff` (the TypeScript arm) and `oracle-layouts` (the
//! d3-hierarchy arm). Split out of `cli.rs` to stay under the house's 300-line limit;
//! shares its shape (`graph_cli`, `gates_dir`, `KNOBS`) rather than importing it, since
//! each file under `tests/` is its own binary crate.

use std::process::{Command, Output};

fn gates_dir() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("gm-cli-oracles-gates-{}", std::process::id()))
}

const KNOBS: [&str; 3] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_NODE_COUNT",
];

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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
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

/// `harness/oracle-layouts.mjs`'s own negative control, inside `cargo test`: emit, run
/// the d3-hierarchy arm clean, then feed it a `layouts.jsonl` one tidy-tree `x` off (the
/// manifest digest fixed up, as a real bug would leave it) and expect red.
#[test]
fn oracle_layouts_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_line() {
    let dir = std::env::temp_dir().join(format!("gm-cli-layouts-{}", std::process::id()));
    let out = dir.to_str().expect("utf-8");
    let emit = graph_cli(&["emit-fixtures", "--seeds", "3", "--out", out], None);
    assert_eq!(emit.status.code(), Some(0), "{}", stdout(&emit));
    let diff = graph_cli(&["oracle-layouts", "--fixtures", out], None);
    assert_eq!(diff.status.code(), Some(0), "{}", stdout(&diff));
    assert!(stdout(&diff).contains("PASS"), "{}", stdout(&diff));
    let record =
        std::fs::read_to_string(gates_dir().join("oracle-layouts.json")).expect("recorded");
    assert!(record.contains("\"pass\": true"), "{record}");

    let text = std::fs::read_to_string(dir.join("layouts.jsonl")).expect("layouts.jsonl");
    let first_line = text.lines().next().expect("at least one seed");
    let value: serde_json::Value = serde_json::from_str(first_line).expect("json line");
    let x0 = value["tidy"]["x"][0].as_f64().expect("tidy.x[0]");
    let from = format!("\"tidy\":{{\"x\":[{}", serde_json::to_string(&x0).unwrap());
    let bumped = x0 + 0.25;
    let to = format!(
        "\"tidy\":{{\"x\":[{}",
        serde_json::to_string(&bumped).unwrap()
    );
    let red = layouts_corrupted(&dir, &text, (&from, &to));
    assert_eq!(red.status.code(), Some(1), "{}", stdout(&red));
    assert!(
        stdout(&red).contains("MISMATCH") && stdout(&red).contains("unexplained mismatches"),
        "{}",
        stdout(&red)
    );

    std::fs::remove_dir_all(&dir).expect("cleanup");
    let missing = graph_cli(&["oracle-layouts", "--fixtures", out], None);
    assert_eq!(
        missing.status.code(),
        Some(2),
        "no fixtures is could-not-run"
    );
}

/// Runs `oracle-layouts` on the fixtures in `dir` with `layouts.jsonl` set to `text` with
/// one line corrupted, and `layout-manifest.json`'s digest updated to match, so only the
/// harness's own comparison can catch it.
fn layouts_corrupted(dir: &std::path::Path, text: &str, (from, to): (&str, &str)) -> Output {
    let (jsonl, manifest) = (dir.join("layouts.jsonl"), dir.join("layout-manifest.json"));
    let wrong = text.replacen(from, to, 1);
    assert_ne!(wrong, text, "a line holding {from:?} to corrupt");
    let before = std::fs::read(&jsonl).expect("layouts.jsonl");
    std::fs::write(&jsonl, &wrong).expect("write");
    let fixed = std::fs::read_to_string(&manifest)
        .expect("manifest")
        .replace(&sha256_hex(&before), &sha256_hex(wrong.as_bytes()));
    std::fs::write(&manifest, fixed).expect("write");
    graph_cli(
        &["oracle-layouts", "--fixtures", dir.to_str().expect("utf-8")],
        None,
    )
}
