//! The differentials whose reference arm is Python, run in the `ge-python-oracle` image
//! (numpy 2.3.3, scipy 1.16.2, networkx 3.6), which the Rust image cannot host. Three
//! steps per [`Differential`], named for it (`spectral`, `fa2`):
//!
//! 1. `emit-<name>-fixtures` writes `<name>.jsonl` (the gate model's edges and our
//!    layouts' coordinates, one line per seed) and `<name>-manifest.json`;
//! 2. `harness/oracle-<name>.py` compares them and writes `<name>-result.json`;
//! 3. `oracle-<name>` checks the result against the differential's ceilings and records
//!    `oracle-<name>.json` for the ledger.
//!
//! Agreement is a tolerance, not bytes: the arms sum in different orders and run
//! different BLAS builds. Each ceiling is measured, then rounded up to the next power of
//! ten above the worst case over 1000 seeds (`docs/measurements/phase06-*.md`).
//!
//! Ponytail: the gate model is one connected random graph per seed of 2 to 601 nodes,
//! so the comparison never meets a disconnected graph or a larger one; those rest on
//! graph-core's own tests.

mod fa2;
mod spectral;

pub use fa2::FA2;
pub use spectral::SPECTRAL;

use crate::evidence::{FINGERPRINTED, Stamp};
use crate::runner::file_sha256;
use graph_contract::geometry::NodeGeometry;
use graph_core::{EdgeRecord, NodeRecord, registry, run_with};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// One Python-armed differential.
pub struct Differential {
    /// Names its files, its subcommands and its ledger record `oracle-<name>`.
    pub name: &'static str,
    /// Ledger id, oracle-result key and the ceiling on its worst case.
    pub ceilings: &'static [(&'static str, &'static str, f64)],
    /// The fixture line for one seed. The second argument is the emit's `--max-iter`
    /// override, or `None` for the differential's own iteration budget.
    pub line: fn(u32, Option<u32>) -> Result<Value, String>,
}

/// `id`'s registered run over the model, as `{x, y}` columns.
fn coords(id: &str, nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<Value, String> {
    let layout = registry::find(id).ok_or_else(|| format!("{id}: not registered"))?;
    let run = run_with(nodes, edges, layout.id, layout.run).map_err(|e| e.to_string())?;
    points(id, &run.snapshot.parts().nodes)
}

/// A layout's node geometry as `{x, y}` columns. Only a point layout is a coordinate
/// comparison; anything else is named rather than silently compared as something else.
pub(super) fn points(id: &str, nodes: &NodeGeometry) -> Result<Value, String> {
    match nodes {
        NodeGeometry::Point { x, y } => Ok(json!({ "x": x, "y": y })),
        other => Err(format!("{id}: expected Point geometry, got {other:?}")),
    }
}

/// `graph-cli emit-<name>-fixtures`.
pub fn emit(
    differential: &Differential,
    seeds: u32,
    max_iter: Option<u32>,
    out: &Path,
) -> ExitCode {
    match write(differential, seeds, max_iter, out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("emit-{}-fixtures: {err}", differential.name);
            ExitCode::from(2)
        }
    }
}

fn write(
    differential: &Differential,
    seeds: u32,
    max_iter: Option<u32>,
    out: &Path,
) -> Result<(), String> {
    let stamp = Stamp::take()?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let name = differential.name;
    let fixtures = format!("{name}.jsonl");
    let path = out.join(&fixtures);
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    );
    for seed in 0..seeds {
        let text = (differential.line)(seed, max_iter).map_err(|e| format!("seed {seed}: {e}"))?;
        writeln!(file, "{text}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    file.flush()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest = json!({
        "seeds": seeds,
        "sha256": { fixtures: file_sha256(&path)? },
        "fingerprint": stamp.fingerprint(),
        "fingerprinted": FINGERPRINTED,
    });
    stamp.still_current()?;
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(out.join(format!("{name}-manifest.json")), text + "\n")
        .map_err(|e| e.to_string())
}

/// `graph-cli oracle-<name>`: reads the harness's result, records the verdict.
pub fn ingest(differential: &Differential, dir: &Path) -> ExitCode {
    match verdict(differential, dir) {
        Ok(pass) => ExitCode::from(u8::from(!pass)),
        Err(err) => {
            eprintln!("oracle-{}: could not run: {err}", differential.name);
            ExitCode::from(2)
        }
    }
}

fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn verdict(differential: &Differential, dir: &Path) -> Result<bool, String> {
    let stamp = Stamp::take()?;
    let name = differential.name;
    let manifest = read(&dir.join(format!("{name}-manifest.json")))?;
    let result = read(&dir.join(format!("{name}-result.json")))?;
    if manifest["fingerprint"] != result["fingerprint"]
        || manifest["fingerprint"].as_str() != Some(stamp.fingerprint())
    {
        return Err("fixtures, result and tree are not the same tree: re-emit and re-run".into());
    }
    if manifest["sha256"][format!("{name}.jsonl")] != result["sha256"] {
        return Err("the result was computed from other fixtures than these".into());
    }
    let (pass, functions) = judge(differential.ceilings, &result)?;
    let body = json!({
        "seeds": manifest["seeds"], "pass": pass, "functions": functions,
        "oracle": result["oracle"], "tolerance": true,
    });
    stamp.still_current()?;
    crate::evidence::write(&stamp, &format!("oracle-{name}"), body)?;
    println!("{}", if pass { "PASS" } else { "FAIL" });
    Ok(pass)
}

/// Each layout's verdict against its ceiling: `(all pass, ledger function entries)`. A
/// layout with no compared case fails: a differential over nothing proves nothing.
fn judge(
    ceilings: &[(&str, &str, f64)],
    result: &Value,
) -> Result<(bool, serde_json::Map<String, Value>), String> {
    let mut pass = true;
    let mut functions = serde_json::Map::new();
    for &(id, key, allowed) in ceilings {
        let row = &result["layouts"][key];
        let cases = row["cases"].as_u64().unwrap_or(0);
        let worst = row["worst"]
            .as_f64()
            .ok_or(format!("{key}: no measured worst"))?;
        let within = cases > 0 && worst <= allowed;
        let verdict = if within { "ok" } else { "FAIL" };
        println!("  {id}: {cases} cases, worst {worst:.3e}, ceiling {allowed:.0e}: {verdict}");
        pass &= within;
        functions.insert(
            id.into(),
            json!({
                "cases": cases, "declared": 0, "unexplained": u64::from(!within),
                "worst": worst, "ceiling": allowed,
            }),
        );
    }
    Ok((pass, functions))
}

#[cfg(test)]
mod tests;
