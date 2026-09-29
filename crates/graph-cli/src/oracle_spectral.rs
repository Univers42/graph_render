//! Differential of `layout.spectral` and `layout.pivot_mds` against SciGraphs' own
//! networkx/scipy implementations (`harness/oracle-spectral.py`, run in the
//! `ge-python-oracle` image, whose scipy is 1.16.2). Three steps, because the Python arm
//! cannot run inside the Rust image:
//!
//! 1. `emit-spectral-fixtures` writes `spectral.jsonl` (the gate model's edges and both
//!    layouts' coordinates, one line per seed) and `spectral-manifest.json`;
//! 2. the harness compares them and writes `spectral-result.json`;
//! 3. `oracle-spectral` checks the result against the ceilings below and records
//!    `oracle-spectral.json` for the ledger.
//!
//! Agreement is a tolerance, not bytes: the two run different BLAS and LOBPCG builds.
//! The ceilings are measured, then rounded up to the next power of ten above the worst
//! component over 1000 seeds (`docs/measurements/phase06-eigen.md`).
//!
//! Ponytail: the gate model is one connected random graph per seed, so the comparison
//! never meets a degenerate eigenspace or a disconnected component; those rest on the
//! closed-form spectra in graph-core's own tests. The spectral metric is a subspace
//! angle, so it cannot see a reflection or a rotation inside the 2D span.

use crate::evidence::{FINGERPRINTED, Stamp};
use crate::runner::file_sha256;
use graph_contract::geometry::NodeGeometry;
use graph_core::{
    REFERENCE_DEGREE, gate_node_count, index_model, registry, run_with, seeded_model,
};
use serde_json::{Value, json};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

/// Ledger id, oracle-result key and the ceiling on its worst component.
pub const CEILINGS: [(&str, &str, f64); 2] = [
    ("layout.spectral", "spectral", 1e-5),
    ("layout.pivot_mds", "pivot_mds", 1e-7),
];

fn coords(
    id: &'static str,
    nodes: &[graph_core::NodeRecord],
    edges: &[graph_core::EdgeRecord],
) -> Result<Value, String> {
    let layout = registry::find(id).ok_or_else(|| format!("{id}: not registered"))?;
    let run = run_with(nodes, edges, id, layout.run).map_err(|e| e.to_string())?;
    match &run.snapshot.parts().nodes {
        NodeGeometry::Point { x, y } => Ok(json!({ "x": x, "y": y })),
        other => Err(format!("{id}: expected Point geometry, got {other:?}")),
    }
}

fn line(seed: u32) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let mut out =
        json!({ "seed": seed, "n": n, "source": columns.source, "target": columns.target });
    for (id, key, _) in CEILINGS {
        out[key] = coords(id, &nodes, &edges)?;
    }
    Ok(out)
}

/// `graph-cli emit-spectral-fixtures`.
pub fn emit(seeds: u32, out: &Path) -> ExitCode {
    match write(seeds, out) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("emit-spectral-fixtures: {err}");
            ExitCode::from(2)
        }
    }
}

fn write(seeds: u32, out: &Path) -> Result<(), String> {
    let stamp = Stamp::take()?;
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join("spectral.jsonl");
    let mut file = std::io::BufWriter::new(
        std::fs::File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    );
    for seed in 0..seeds {
        let text = line(seed).map_err(|e| format!("seed {seed}: {e}"))?;
        writeln!(file, "{text}").map_err(|e| format!("{}: {e}", path.display()))?;
    }
    file.flush()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest = json!({
        "seeds": seeds,
        "sha256": { "spectral.jsonl": file_sha256(&path)? },
        "fingerprint": stamp.fingerprint(),
        "fingerprinted": FINGERPRINTED,
    });
    stamp.still_current()?;
    let text = serde_json::to_string_pretty(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(out.join("spectral-manifest.json"), text + "\n").map_err(|e| e.to_string())
}

/// `graph-cli oracle-spectral`: reads the harness's result, records the verdict.
pub fn ingest(dir: &Path) -> ExitCode {
    match verdict(dir) {
        Ok(pass) => ExitCode::from(u8::from(!pass)),
        Err(err) => {
            eprintln!("oracle-spectral: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

fn read(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

fn verdict(dir: &Path) -> Result<bool, String> {
    let stamp = Stamp::take()?;
    let manifest = read(&dir.join("spectral-manifest.json"))?;
    let result = read(&dir.join("spectral-result.json"))?;
    if manifest["fingerprint"] != result["fingerprint"]
        || manifest["fingerprint"].as_str() != Some(stamp.fingerprint())
    {
        return Err("fixtures, result and tree are not the same tree: re-emit and re-run".into());
    }
    if manifest["sha256"]["spectral.jsonl"] != result["sha256"] {
        return Err("the result was computed from other fixtures than these".into());
    }
    let (pass, functions) = judge(&result)?;
    let body = json!({
        "seeds": manifest["seeds"], "pass": pass, "functions": functions,
        "oracle": result["oracle"], "tolerance": true,
    });
    stamp.still_current()?;
    crate::evidence::write(&stamp, "oracle-spectral", body)?;
    println!("{}", if pass { "PASS" } else { "FAIL" });
    Ok(pass)
}

/// Each layout's verdict against its ceiling: `(all pass, ledger function entries)`. A
/// layout with no compared component fails: a differential over nothing proves nothing.
fn judge(result: &Value) -> Result<(bool, serde_json::Map<String, Value>), String> {
    let mut pass = true;
    let mut functions = serde_json::Map::new();
    for (id, key, allowed) in CEILINGS {
        let row = &result["layouts"][key];
        let cases = row["cases"].as_u64().unwrap_or(0);
        let worst = row["worst"]
            .as_f64()
            .ok_or(format!("{key}: no measured worst"))?;
        let within = cases > 0 && worst <= allowed;
        let verdict = if within { "ok" } else { "FAIL" };
        println!("  {id}: {cases} components, worst {worst:.3e}, ceiling {allowed:.0e}: {verdict}");
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
mod tests {
    use super::*;

    fn result(spectral: (u64, f64), pivot: (u64, f64)) -> Value {
        json!({ "layouts": {
            "spectral": { "cases": spectral.0, "worst": spectral.1 },
            "pivot_mds": { "cases": pivot.0, "worst": pivot.1 },
        }})
    }

    #[test]
    fn a_worst_at_or_under_its_ceiling_passes_and_records_it() {
        let (pass, functions) = judge(&result((9, 1e-5), (4, 0.0))).expect("judged");
        assert!(pass);
        assert_eq!(functions["layout.spectral"]["cases"], 9);
        assert_eq!(functions["layout.spectral"]["unexplained"], 0);
        assert_eq!(functions["layout.pivot_mds"]["ceiling"], 1e-7);
    }

    #[test]
    fn a_worst_over_its_ceiling_or_no_component_fails_that_layout_only() {
        let (pass, functions) = judge(&result((9, 1.1e-5), (4, 0.0))).expect("judged");
        assert!(!pass);
        assert_eq!(functions["layout.spectral"]["unexplained"], 1);
        assert_eq!(functions["layout.pivot_mds"]["unexplained"], 0);
        let (pass, functions) = judge(&result((9, 0.0), (0, 0.0))).expect("judged");
        assert!(!pass);
        assert_eq!(functions["layout.pivot_mds"]["unexplained"], 1);
    }

    #[test]
    fn a_result_without_a_measured_worst_is_refused() {
        assert!(judge(&json!({ "layouts": {} })).is_err());
    }
}
