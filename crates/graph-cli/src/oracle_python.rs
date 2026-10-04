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

mod basic_3d;
mod circo;
mod circular_hierarchy;
mod cli;
mod closed_form;
pub mod conformance;
mod fa2;
mod fdp;
mod graphviz;
mod hierarchical_3d;
mod igraph;
mod judge;
mod neato;
mod osage;
mod patchwork;
mod scale;
mod sfdp;
mod spectral;
pub mod spring;
mod twopi;

pub use basic_3d::BASIC_3D;
pub use circular_hierarchy::CIRCULAR_HIERARCHY;
pub use cli::Cli;

pub use closed_form::CLOSED_FORM;
pub use fa2::FA2;
pub use hierarchical_3d::HIERARCHICAL_3D;
pub use igraph::{IGRAPH, IGRAPH_3D};
pub use scale::SCALE;
pub use spectral::SPECTRAL;
pub use spring::SPRING;

use crate::evidence::{FINGERPRINTED, Stamp};
use crate::runner::file_sha256;
use graph_contract::geometry::NodeGeometry;
use graph_core::{EdgeRecord, NodeRecord, registry, run_with};
use judge::{closed_cases, judge, unbroken};
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

/// A 3D layout's geometry as `{x, y, z}` columns, from the **snapshot** rather than the
/// geometry.
///
/// The `z` is read off `SnapshotParts::z`, which is where a 3D layout's third column
/// actually lands (`layout::snapshot` writes it there and nowhere else), so this compares
/// what the wire carries rather than what the layout returned — a `Geometry::in_space` whose
/// z never reached the snapshot would answer every `x` and `y` correctly and be caught here
/// as a missing column rather than as a silent 2D comparison.
///
/// A missing or empty-against-nodes `z` is **refused**, not defaulted: `None` on a 3D
/// layout means the column was dropped somewhere between the layout and the wire, which is
/// the exact failure the 3D rows exist to make loud.
pub(super) fn columns_3d(
    id: &str,
    nodes: &NodeGeometry,
    z: Option<&[f32]>,
) -> Result<Value, String> {
    let NodeGeometry::Point { x, y } = nodes else {
        return Err(format!("{id}: expected Point geometry, got {nodes:?}"));
    };
    let Some(z) = z else {
        return Err(format!(
            "{id}: a 3D layout with no z column in its snapshot"
        ));
    };
    if z.len() != x.len() || z.len() != y.len() {
        return Err(format!(
            "{id}: z has {} values against {} nodes — the column was dropped or truncated",
            z.len(),
            x.len()
        ));
    }
    Ok(json!({ "x": x, "y": y, "z": z }))
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
    let (mut pass, functions) = judge(differential.ceilings, &result)?;
    pass &= closed_cases(&result);
    pass &= unbroken(&result);
    let body = json!({
        "seeds": manifest["seeds"], "pass": pass, "functions": functions,
        "oracle": result["oracle"],
        // A differential whose arms return the same bytes rather than agreeing within a
        // measured ceiling says so in its own result; one that says nothing keeps the
        // reading every record before it was written with.
        "tolerance": result.get("tolerance").and_then(Value::as_bool).unwrap_or(true),
    });
    stamp.still_current()?;
    crate::evidence::record(&stamp, &format!("oracle-{name}"), body)?;
    println!("{}", if pass { "PASS" } else { "FAIL" });
    Ok(pass)
}

#[cfg(test)]
mod tests;
