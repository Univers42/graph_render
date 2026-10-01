//! `snapshot` and `roundtrip`: the snapshot contract's two faces, emitted and checked.
//!
//! `snapshot` is what a third-party consumer runs: one seed's model through a registered
//! layout, written as the binary face, the canonical JSON face, or both.
//!
//! `roundtrip` ([`roundtrip::run`]) checks, seed by seed, that the two faces carry one
//! snapshot byte for byte, with no tolerance anywhere: binary → JSON → binary and JSON →
//! binary → JSON return what they started from, and every float read the way a
//! JavaScript consumer reads it (`JSON.parse` to f64, then `Math.fround`) keeps its bits.

mod dag;
mod exercise;
mod hand_oracles;
mod roundtrip;

use crate::runner::sha256_hex;
use graph_contract::binary::Snapshot;
use graph_contract::canonical_json::{EDGE_KINDS, NODE_KINDS, from_json, to_json};
use graph_contract::geometry::EdgeGeometry;
use graph_core::{
    PipelineRun, REFERENCE_DEGREE, gate_node_count, registry, run_with, seeded_model,
};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub use roundtrip::run as roundtrip;

/// Most nodes `snapshot` builds: the synthetic model's own limit.
pub const MAX_NODES: i64 = 100_000;

/// Every value `snapshot --layout` accepts, in registry order: each registered layout's
/// full id and its short name, so either form works. A layout whose id already *is* its
/// own short name contributes it once, so no name is ever offered twice.
pub fn layout_names() -> Vec<&'static str> {
    let mut names = Vec::with_capacity(2 * registry::LAYOUTS.len());
    for layout in &registry::LAYOUTS {
        names.push(layout.id);
        let short = short_name(layout.id);
        if short != layout.id {
            names.push(short);
        }
    }
    names
}

/// A layout id without its `layout.` prefix — the form `--layout` is documented in, and
/// the one rule [`layout_names`] and [`pipeline`] share rather than each spelling out.
pub fn short_name(id: &str) -> &str {
    id.strip_prefix("layout.").unwrap_or(id)
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

/// The pipeline over the gate's model for `seed` at `nodes`, through layout `name`:
/// either its short name or its full id.
fn pipeline(seed: u32, nodes: u32, name: &str) -> Result<PipelineRun, String> {
    let id = format!("layout.{}", short_name(name));
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
    // `columns_dim`, so a 3D snapshot's z column is narrowed through f64 like every other
    // float and not skipped: it is a node column like x and y.
    let mut columns: Vec<(String, &[f32])> = (p.nodes.columns_dim(p.z.as_deref()).into_iter())
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

#[cfg(test)]
mod tests;
