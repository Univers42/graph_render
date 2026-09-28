//! `snapshot` and `roundtrip`: the snapshot contract's two faces, emitted and checked.
//!
//! `snapshot` is what a third-party consumer runs: one seed's model through a registered
//! layout, written as the binary face, the canonical JSON face, or both.
//!
//! `roundtrip` checks, seed by seed, that the two faces carry one snapshot byte for byte,
//! with no tolerance anywhere: binary → JSON → binary and JSON → binary → JSON return
//! what they started from, and every float read the way a JavaScript consumer reads it
//! (`JSON.parse` to f64, then `Math.fround`) keeps its bits. Each seed checks two
//! snapshots: the grid pipeline's, and a contract exercise drawing every kind,
//! adversarial floats and ids from the seed. The grid's is also held to the grid's
//! conventions restated in f64 — the hand oracle `layout.grid` is gated on — and the run
//! is recorded in `target/gates/roundtrip.json` for the ledger.

mod exercise;

use crate::evidence;
use crate::runner::sha256_hex;
use graph_contract::binary::Snapshot;
use graph_contract::canonical_json::{EDGE_KINDS, NODE_KINDS, from_json, to_json};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_core::{
    PipelineRun, REFERENCE_DEGREE, gate_node_count, registry, run_with, seeded_model,
};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Most nodes `snapshot` builds: the synthetic model's own limit.
pub const MAX_NODES: i64 = 100_000;

/// Every name `snapshot --layout` accepts: each registered layout id without `layout.`.
pub fn layout_names() -> Vec<&'static str> {
    let ids = registry::LAYOUTS.iter().map(|layout| layout.id);
    ids.filter_map(|id| id.strip_prefix("layout.")).collect()
}

/// Where `snapshot` writes each face; `-` is standard output.
#[derive(Debug, Clone, Default)]
pub struct Outputs {
    /// The binary face.
    pub bin: Option<PathBuf>,
    /// The canonical JSON face.
    pub json: Option<PathBuf>,
}

/// `snapshot`: the summary goes to standard error, so either face can be piped.
pub fn snapshot(seed: u32, nodes: Option<u32>, layout: &str, out: &Outputs) -> ExitCode {
    let nodes = nodes.unwrap_or(gate_node_count(seed));
    match write_faces(seed, nodes, layout, out) {
        Ok(summary) => {
            eprint!("{summary}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("snapshot: {err}");
            ExitCode::from(2)
        }
    }
}

/// The pipeline over the gate's model for `seed` at `nodes`, through layout `name`.
fn pipeline(seed: u32, nodes: u32, name: &str) -> Result<PipelineRun, String> {
    let id = format!("layout.{name}");
    let known = || layout_names().join(", ");
    let layout =
        registry::find(&id).ok_or_else(|| format!("no layout {name:?}: one of {}", known()))?;
    let (nodes, edges) = seeded_model(seed, nodes, REFERENCE_DEGREE);
    run_with(&nodes, &edges, layout.id, layout.run).map_err(|err| format!("{id}: {err}"))
}

fn write_faces(seed: u32, nodes: u32, layout: &str, out: &Outputs) -> Result<String, String> {
    let stdout = Some(Path::new("-"));
    match (out.bin.as_deref(), out.json.as_deref()) {
        (None, None) => return Err("nothing to write: pass --out-bin, --out-json, or both".into()),
        (bin, json) if bin == stdout && json == stdout => {
            return Err("only one face can go to standard output".into());
        }
        _ => {}
    }
    let run = pipeline(seed, nodes, layout)?;
    let h = run.snapshot.header();
    let node_kind = NODE_KINDS
        .iter()
        .find(|(k, _)| *k == h.node_kind)
        .map_or("?", |k| k.1);
    let edge_kind = EDGE_KINDS
        .iter()
        .find(|(k, _)| *k == h.edge_kind)
        .map_or("?", |k| k.1);
    let mut summary = format!(
        "snapshot: seed {seed}, {}, {} nodes, {} edges, {node_kind}/{edge_kind}, format {}\n",
        run.layout, h.node_count, h.edge_count, h.version
    );
    if let Some(path) = &out.bin {
        let bytes = run.snapshot.to_bytes();
        emit(path, &bytes)?;
        summary += &format!(
            "  binary {} ({} bytes, sha256 {})\n",
            path.display(),
            bytes.len(),
            sha256_hex(&bytes)
        );
    }
    if let Some(path) = &out.json {
        let text = to_json(&run.snapshot);
        emit(path, text.as_bytes())?;
        summary += &format!("  json   {} ({} bytes)\n", path.display(), text.len());
    }
    Ok(summary)
}

fn emit(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let written = if path == Path::new("-") {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(bytes).and_then(|()| stdout.flush())
    } else {
        std::fs::write(path, bytes)
    };
    written.map_err(|err| format!("writing {}: {err}", path.display()))
}

/// What the sweep found wrong, by check. Both empty is a pass.
#[derive(Debug, Default)]
struct Findings {
    /// Snapshots whose faces did not round-trip.
    faces: Vec<String>,
    /// Seeds whose grid is off its conventions.
    grid: Vec<String>,
}

/// `roundtrip --seeds N`.
pub fn roundtrip(seeds: u32) -> ExitCode {
    let swept = evidence::Stamp::take().and_then(|stamp| Ok((stamp, sweep(seeds)?)));
    let (stamp, found) = match swept {
        Ok(swept) => swept,
        Err(err) => {
            eprintln!("roundtrip: could not run: {err}");
            return ExitCode::from(2);
        }
    };
    let pass = all_clear(&found);
    print_findings(seeds, &found);
    let grid = json!({ "cases": seeds, "declared": 0, "unexplained": found.grid.len() });
    let body = json!({
        "seeds": seeds, "pass": pass, "snapshots": 2 * u64::from(seeds),
        "faces_failed": found.faces.len(), "functions": { "layout.grid": grid }
    });
    if let Err(err) = evidence::write(&stamp, "roundtrip", body) {
        eprintln!("roundtrip: not recorded: {err}");
        return ExitCode::from(2);
    }
    println!("{}", if pass { "PASS" } else { "FAIL" });
    ExitCode::from(if pass { 0 } else { 1 })
}

fn sweep(seeds: u32) -> Result<Findings, String> {
    if seeds == 0 {
        return Err("0 seeds: a sweep over nothing proves nothing".into());
    }
    let mut found = Findings::default();
    for seed in 0..seeds {
        let grid = pipeline(seed, gate_node_count(seed), "grid")?.snapshot;
        for (what, snapshot) in [("grid", &grid), ("exercise", &exercise::snapshot(seed)?)] {
            if let Err(why) = faces_agree(snapshot) {
                found.faces.push(format!("seed {seed} {what}: {why}"));
            }
        }
        if let Err(why) = grid_by_hand(&grid) {
            found.grid.push(format!("seed {seed}: {why}"));
        }
    }
    Ok(found)
}

/// Both checks clean: the only way `roundtrip` passes.
fn all_clear(found: &Findings) -> bool {
    found.faces.is_empty() && found.grid.is_empty()
}

fn print_findings(seeds: u32, found: &Findings) {
    let mut text = String::new();
    write_findings(&mut text, seeds, found);
    print!("{text}");
}

/// `print_findings`'s text, built in memory so the counts it reports can be checked.
fn write_findings(out: &mut String, seeds: u32, found: &Findings) {
    use std::fmt::Write as _;
    let snapshots = 2 * u64::from(seeds);
    let _ = writeln!(
        out,
        "roundtrip: seeds={seeds} snapshots={snapshots} (grid pipeline + contract exercise)"
    );
    let faces_ok = snapshots - found.faces.len() as u64;
    let _ = writeln!(
        out,
        "  binary <-> JSON byte-exact on {faces_ok}/{snapshots} snapshots"
    );
    let grid_ok = u64::from(seeds) - found.grid.len() as u64;
    let _ = writeln!(
        out,
        "  layout.grid on its stated conventions on {grid_ok}/{seeds} seeds"
    );
    for line in found.faces.iter().chain(&found.grid).take(6) {
        let _ = writeln!(out, "  FAILED {line}");
    }
}

/// Both directions of the round trip, and the JavaScript reading of every float.
fn faces_agree(snapshot: &Snapshot) -> Result<(), String> {
    let bytes = snapshot.to_bytes();
    let decoded =
        Snapshot::from_bytes(&bytes).map_err(|e| format!("binary does not read back: {e}"))?;
    if decoded != *snapshot {
        return Err("the binary reads back as another snapshot".into());
    }
    let text = to_json(&decoded);
    let parsed = from_json(&text).map_err(|e| format!("JSON does not read back: {e}"))?;
    let rebuilt = parsed.to_bytes();
    if rebuilt != bytes {
        return Err("binary -> JSON -> binary changed the bytes".into());
    }
    let reread = Snapshot::from_bytes(&rebuilt).map_err(|e| format!("rebuilt binary: {e}"))?;
    if to_json(&reread) != text {
        return Err("JSON -> binary -> JSON changed the text".into());
    }
    floats_survive_f64(&decoded)
}

/// Every float as the JSON face writes it, read to f64 and narrowed, keeps its bits.
fn floats_survive_f64(snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let mut columns: Vec<(String, &[f32])> = (p.nodes.columns().into_iter())
        .map(|(name, column)| (format!("node.{name}"), column))
        .collect();
    if let EdgeGeometry::Polyline(paths) | EdgeGeometry::Curve { paths, .. } = &p.edges {
        columns.push(("edge.pts".into(), &paths.pts));
    }
    for (column, values) in columns {
        for (i, v) in values.iter().enumerate() {
            let text = v.to_string();
            let read = text.parse::<f64>().map_or(f32::NAN, |wide| wide as f32);
            if read.to_bits() != v.to_bits() {
                return Err(format!(
                    "{column}[{i}] written {text} reads back through f64 as {read}"
                ));
            }
        }
    }
    Ok(())
}

/// The grid's conventions restated in f64, independently of graph-core's integer
/// `dimensions`: `cols = ceil(sqrt(n))`, `rows = ceil(n / cols)`, node `i` in cell
/// `(i mod cols, floor(i / cols))`, the lattice centred on the origin at unit spacing,
/// `Point` nodes and `Line` edges. Compared bit for bit.
fn grid_by_hand(snapshot: &Snapshot) -> Result<(), String> {
    let p = snapshot.parts();
    let (NodeGeometry::Point { x, y }, EdgeGeometry::Line) = (&p.nodes, &p.edges) else {
        return Err("not Point nodes with Line edges".into());
    };
    let n = x.len() as f64;
    let cols = n.sqrt().ceil();
    let rows = (n / cols).ceil();
    for (i, (&gx, &gy)) in x.iter().zip(y).enumerate() {
        let (col, row) = (i as f64 % cols, (i as f64 / cols).floor());
        let want = (
            (col - (cols - 1.0) / 2.0) as f32,
            (row - (rows - 1.0) / 2.0) as f32,
        );
        if (gx.to_bits(), gy.to_bits()) != (want.0.to_bits(), want.1.to_bits()) {
            return Err(format!(
                "node {i} at ({gx}, {gy}), the conventions put it at {want:?}"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
