//! The two oracle differentials end to end, each with its own negative control inside
//! `cargo test`: `oracle-diff` (the TypeScript arm) and `oracle-layouts` (the
//! d3-hierarchy arm). Split out of `cli.rs` to stay under the house's 300-line limit;
//! shares its shape (`graph_cli`, `gates_dir`, `KNOBS`) rather than importing it, since
//! each file under `tests/` is its own binary crate.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn gates_dir() -> PathBuf {
    std::env::temp_dir().join(format!("gm-cli-oracles-gates-{}", std::process::id()))
}

const KNOBS: [&str; 3] = [
    "GM_MUTATE_REFERENCE_DEGREE",
    "GM_MUTATE_GRID_SPACING",
    "GM_MUTATE_NODE_COUNT",
];

/// `graph-cli` with every mutation knob cleared — so a control can never be inherited
/// from the environment — and its verdict recorded under `gates`, which each test names
/// for itself: two tests driving one gate at once would read each other's record.
fn graph_cli(gates: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_graph-cli"));
    command.args(args).env("GM_GATES_DIR", gates);
    for knob in KNOBS {
        command.env_remove(knob);
    }
    command.output().expect("graph-cli runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The oracle differential end to end on what `emit-fixtures` wrote: the TypeScript arm
/// runs it and finds nothing unexplained, and records that verdict.
#[test]
fn oracle_diff_passes_on_emitted_fixtures_and_records_the_verdict() {
    let gates = gates_dir().join("diff");
    let (dir, out) = emit_fixtures(&gates, "diff");
    let diff = graph_cli(&gates, &["oracle-diff", "--fixtures", &out]);
    assert_eq!(diff.status.code(), Some(0), "{}", stdout(&diff));
    assert!(stdout(&diff).contains("0 unexplained") && stdout(&diff).ends_with("PASS\n"));
    let record = std::fs::read_to_string(gates.join("oracle-diff.json")).expect("recorded");
    assert!(record.contains("\"pass\": true"), "{record}");
    std::fs::remove_dir_all(&dir).expect("cleanup");
    let missing = graph_cli(&gates, &["oracle-diff", "--fixtures", &out]);
    assert_eq!(
        missing.status.code(),
        Some(2),
        "no fixtures is could-not-run"
    );
}

/// The same arm's negative control, inside `cargo test`: a graph-core line one byte off
/// (manifest digest fixed up, as a real bug would leave it) must go red, and so must a
/// group off by 256 — H9's rule accepts graph-core's group only where it is the
/// untruncated index.
#[test]
fn oracle_diff_goes_red_on_a_wrong_line_and_on_a_group_off_by_256() {
    let gates = gates_dir().join("diff-red");
    let (dir, out) = emit_fixtures(&gates, "diff-red");
    let text = std::fs::read_to_string(dir.join("expect.jsonl")).expect("expect.jsonl");
    let red = diff_corrupted(&gates, &dir, &text, ("\"notes\":0}", "\"notes\":1}"));
    assert_eq!(
        red.status.code(),
        Some(1),
        "{}{}",
        stdout(&red),
        stderr(&red)
    );
    assert!(stdout(&red).contains("MISMATCH") && stdout(&red).contains("FAIL: 1 unexplained"));
    let off = diff_corrupted(
        &gates,
        &dir,
        &text,
        ("\n[0,0,0,0,0,0]\n", "\n[256,0,0,0,0,0]\n"),
    );
    assert_eq!(off.status.code(), Some(1), "{}", stdout(&off));
    assert!(stdout(&off).contains("MISMATCH line") && stdout(&off).contains("layoutGroups"));
    std::fs::remove_dir_all(&dir).expect("cleanup");
    let _ = out;
}

/// `seeds` of the topology oracle's fixtures in `name`'s own directory, and the path to
/// read them from.
fn emit_fixtures(gates: &Path, name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("gm-cli-{name}-fixtures-{}", std::process::id()));
    let out = dir.to_str().expect("utf-8").to_owned();
    let emit = graph_cli(gates, &["emit-fixtures", "--seeds", "3", "--out", &out]);
    assert_eq!(emit.status.code(), Some(0), "{}", stdout(&emit));
    (dir, out)
}

/// Runs the harness on the fixtures in `dir` with `expect.jsonl` set to `text` with one
/// line corrupted, and the manifest's digest updated to match, so only the harness's
/// comparison can catch it.
fn diff_corrupted(gates: &Path, dir: &Path, text: &str, (from, to): (&str, &str)) -> Output {
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
        gates,
        &["oracle-diff", "--fixtures", dir.to_str().expect("utf-8")],
    )
}

/// `harness/oracle-layouts.mjs`'s own negative control on the tidy arm, inside `cargo
/// test`: emit, run the d3-hierarchy arm clean, then feed it a `layouts.jsonl` one
/// tidy-tree `x` off (the manifest digest fixed up, as a real bug would leave it) and
/// expect red.
#[test]
fn oracle_layouts_passes_on_emitted_fixtures_and_goes_red_on_a_wrong_tidy_x() {
    let gates = gates_dir().join("tidy");
    let (dir, out) = emit_layouts(&gates, "tidy");
    let diff = graph_cli(&gates, &["oracle-layouts", "--fixtures", &out]);
    assert_eq!(diff.status.code(), Some(0), "{}", stdout(&diff));
    assert!(stdout(&diff).contains("PASS"), "{}", stdout(&diff));
    let record = std::fs::read_to_string(gates.join("oracle-layouts.json")).expect("recorded");
    assert!(record.contains("\"pass\": true"), "{record}");

    let text = std::fs::read_to_string(dir.join("layouts.jsonl")).expect("layouts.jsonl");
    let first = text.lines().next().expect("at least one seed");
    let x0 = serde_json::from_str::<serde_json::Value>(first).expect("json line")["tidy"]["x"][0]
        .as_f64()
        .expect("tidy.x[0]");
    const TIDY_X: &str = "\"tidy\":{\"x\":[";
    let from = first_of(TIDY_X, x0);
    let to = first_of(TIDY_X, x0 + 0.25);
    let red = layouts_corrupted(&gates, &dir, &text, (&from, &to));
    assert_eq!(
        red.status.code(),
        Some(1),
        "{}{}",
        stdout(&red),
        stderr(&red)
    );
    assert!(
        stdout(&red).contains("MISMATCH") && stdout(&red).contains("unexplained mismatches"),
        "{}",
        stdout(&red)
    );

    std::fs::remove_dir_all(&dir).expect("cleanup");
    let missing = graph_cli(&gates, &["oracle-layouts", "--fixtures", &out]);
    assert_eq!(
        missing.status.code(),
        Some(2),
        "no fixtures is could-not-run"
    );
}

/// The same control on the treemap arm: one box edge off must turn the harness red too,
/// so a d3-hierarchy differential that compared only the tidy tree — and dropped the
/// whole squarify port — would still be caught here.
#[test]
fn oracle_layouts_goes_red_on_a_wrong_treemap_box_edge() {
    let gates = gates_dir().join("treemap");
    let (dir, out) = emit_layouts(&gates, "treemap");
    let diff = graph_cli(&gates, &["oracle-layouts", "--fixtures", &out]);
    assert_eq!(diff.status.code(), Some(0), "{}", stdout(&diff));
    assert!(stdout(&diff).contains("PASS"), "{}", stdout(&diff));

    let text = std::fs::read_to_string(dir.join("layouts.jsonl")).expect("layouts.jsonl");
    let first = text.lines().next().expect("at least one seed");
    let w0 = serde_json::from_str::<serde_json::Value>(first).expect("json line")["treemap"]["w"]
        [0]
    .as_f64()
    .expect("treemap.w[0]");
    const TREEMAP_W: &str = "\"w\":[";
    let from = first_of(TREEMAP_W, w0);
    let to = first_of(TREEMAP_W, w0 + 0.25);
    let red = layouts_corrupted(&gates, &dir, &text, (&from, &to));
    assert_eq!(
        red.status.code(),
        Some(1),
        "{}{}",
        stdout(&red),
        stderr(&red)
    );
    let out = stdout(&red);
    assert!(
        out.contains("MISMATCH") && out.contains("layout.treemap.squarified"),
        "{out}"
    );
    assert!(out.contains("FAIL: 1 unexplained mismatches"), "{out}");
    std::fs::remove_dir_all(&dir).expect("cleanup");
}

/// The emitted text's opening up to and including `value`: the exact bytes the writer
/// produced, so replacing one with the other changes that number and nothing else.
fn first_of(prefix: &str, value: f64) -> String {
    format!("{prefix}{}", serde_json::to_string(&value).unwrap())
}

/// One seed of layout fixtures in `name`'s own directory, and the path to read them from.
fn emit_layouts(gates: &Path, name: &str) -> (PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("gm-cli-layouts-{name}-{}", std::process::id()));
    let out = dir.to_str().expect("utf-8").to_owned();
    let emit = graph_cli(gates, &["emit-fixtures", "--seeds", "1", "--out", &out]);
    assert_eq!(emit.status.code(), Some(0), "{}", stdout(&emit));
    (dir, out)
}

/// Runs `oracle-layouts` on the fixtures in `dir` with `layouts.jsonl` set to `text` with
/// one line corrupted, and `layout-manifest.json`'s digest updated to match, so only the
/// harness's own comparison can catch it.
fn layouts_corrupted(gates: &Path, dir: &Path, text: &str, (from, to): (&str, &str)) -> Output {
    let (jsonl, manifest) = (dir.join("layouts.jsonl"), dir.join("layout-manifest.json"));
    let wrong = text.replacen(from, to, 1);
    assert_ne!(wrong, text, "a line holding {from:?} to corrupt");
    let before = std::fs::read(&jsonl).expect("layouts.jsonl");
    std::fs::write(&jsonl, &wrong).expect("write");
    let fixed = std::fs::read_to_string(&manifest)
        .expect("manifest")
        .replace(&sha256_hex(&before), &sha256_hex(wrong.as_bytes()));
    std::fs::write(&manifest, fixed).expect("write");
    let out = dir.to_str().expect("utf-8");
    graph_cli(gates, &["oracle-layouts", "--fixtures", out])
}
