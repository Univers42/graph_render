//! `graph-cli emit-fixtures`: the oracle differential's inputs and the Rust arm's
//! outputs, written as data (`prompt.md` §7.4).
//!
//! Three files land in the output directory: `cases.jsonl` (one input per line),
//! `expect.jsonl` (graph-core's canonical output for the case on the same line) and
//! `manifest.json` (generator, seed count, per-function counts, file digests and the
//! tree fingerprint). The expected outputs are computed from `cases.jsonl` *as read
//! back from disk*, so the Rust arm consumes exactly the bytes the TypeScript arm will.

mod cases;
mod eval;
mod generate;
mod wire;

use crate::evidence::{FINGERPRINTED, tree_fingerprint};
use crate::runner::{CHILD_TIMEOUT, file_sha256, run_status, workspace_root};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// The hand-written H1 pairs (`prompts/phase-01-topology.md` step 5).
pub const ADVERSARIAL: &str = "fixtures/adversarial-ids.json";

/// Default output directory, relative to the workspace root.
pub fn default_out() -> PathBuf {
    workspace_root().join("target").join("oracle-fixtures")
}

/// Writes the fixtures for seeds `0..seeds` into `out`.
pub fn run(seeds: u32, out: &Path) -> ExitCode {
    match emit(seeds, out) {
        Ok(counts) => {
            let total: u64 = counts.values().sum();
            println!(
                "emit-fixtures: {total} cases over {seeds} seeds into {}",
                out.display()
            );
            for (function, count) in counts {
                println!("  {function:<20} {count}");
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("emit-fixtures: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// `graph-cli oracle-diff`: the TypeScript arm, exactly as `npm run oracle:diff` runs it,
/// over the fixtures in `fixtures`.
pub fn diff(fixtures: &Path) -> ExitCode {
    let root = workspace_root();
    let mut command = std::process::Command::new("node");
    command
        .current_dir(&root)
        .args(["--experimental-strip-types", "--experimental-loader"])
        .arg(root.join("tests").join("ts-extension-loader.mjs"))
        .arg(root.join("harness").join("oracle-diff.mjs"))
        .arg(fixtures);
    ExitCode::from(diff_code(run_status(&mut command, CHILD_TIMEOUT)))
}

/// The harness's own `0` and `1` pass through; anything else it could end with (`2`, a
/// signal, a missing `node`, a timeout) is "could not run".
fn diff_code(status: Result<std::process::ExitStatus, String>) -> u8 {
    match status {
        Ok(status) => match status.code() {
            Some(0) => 0,
            Some(1) => 1,
            _ => 2,
        },
        Err(err) => {
            eprintln!("oracle-diff: could not run: {err}");
            2
        }
    }
}

fn emit(seeds: u32, out: &Path) -> Result<BTreeMap<String, u64>, String> {
    if seeds == 0 {
        return Err("0 seeds: a differential over nothing proves nothing".into());
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let pairs = adversarial_pairs(&workspace_root().join(ADVERSARIAL))?;
    let cases_path = out.join("cases.jsonl");
    write_cases(&cases_path, seeds, &pairs)?;
    let counts = write_expected(&cases_path, &out.join("expect.jsonl"))?;
    let manifest = json!({
        "generator": "graph-cli emit-fixtures: splitmix64 per seed (oracle_fixtures/generate.rs)",
        "format": 1,
        "seeds": seeds,
        "counts": counts,
        "adversarial": { "path": ADVERSARIAL, "sha256": file_sha256(&workspace_root().join(ADVERSARIAL))? },
        "sha256": {
            "cases.jsonl": file_sha256(&cases_path)?,
            "expect.jsonl": file_sha256(&out.join("expect.jsonl"))?,
        },
        "fingerprint": tree_fingerprint()?,
        "fingerprinted": FINGERPRINTED,
    });
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    let path = out.join("manifest.json");
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(counts)
}

/// The `[a, b]` of every pair in the adversarial fixture.
fn adversarial_pairs(path: &Path) -> Result<Vec<(String, String)>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let pairs = doc["pairs"]
        .as_array()
        .ok_or("adversarial fixture has no `pairs` array")?;
    pairs
        .iter()
        .map(|p| match (p["a"].as_str(), p["b"].as_str()) {
            (Some(a), Some(b)) => Ok((a.to_owned(), b.to_owned())),
            _ => Err(format!("adversarial pair without string `a` and `b`: {p}")),
        })
        .collect()
}

fn create(path: &Path) -> Result<BufWriter<std::fs::File>, String> {
    std::fs::File::create(path)
        .map(BufWriter::new)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn write_cases(path: &Path, seeds: u32, pairs: &[(String, String)]) -> Result<(), String> {
    let mut file = create(path)?;
    let mut line = |seed: Value, case: cases::Case| -> Result<(), String> {
        let record = json!({ "seed": seed, "fn": case.function, "args": case.args });
        writeln!(file, "{record}").map_err(|e| format!("{}: {e}", path.display()))
    };
    for seed in 0..seeds {
        for case in cases::for_seed(seed) {
            line(json!(seed), case)?;
        }
    }
    for case in cases::adversarial(pairs) {
        line(Value::Null, case)?;
    }
    file.flush().map_err(|e| format!("{}: {e}", path.display()))
}

fn write_expected(cases: &Path, expect: &Path) -> Result<BTreeMap<String, u64>, String> {
    let input = std::fs::File::open(cases).map_err(|e| format!("{}: {e}", cases.display()))?;
    let mut output = create(expect)?;
    let mut evaluator = eval::Evaluator::default();
    let mut counts = BTreeMap::new();
    for (number, line) in BufReader::new(input).lines().enumerate() {
        let line = line.map_err(|e| format!("{}: {e}", cases.display()))?;
        let case: Value = serde_json::from_str(&line).map_err(|e| format!("case {number}: {e}"))?;
        let function = case["fn"]
            .as_str()
            .ok_or(format!("case {number} has no fn"))?;
        let result = evaluator
            .eval(function, &case["args"])
            .map_err(|e| format!("case {number} ({function}): {e}"))?;
        writeln!(output, "{result}").map_err(|e| format!("{}: {e}", expect.display()))?;
        if function != "graph" {
            *counts.entry(function.to_owned()).or_insert(0) += 1;
        }
    }
    output
        .flush()
        .map_err(|e| format!("{}: {e}", expect.display()))?;
    Ok(counts)
}

#[cfg(test)]
mod tests;
