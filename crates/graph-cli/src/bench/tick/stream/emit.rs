//! Writing the stream file: `--emit` turns the scale model into line 0 plus `K` batch lines,
//! one JSON v1 document per line, every line by `graph_wasm::ingest_document` — the same
//! encoder `gm_build` and `gm_graph_extend` read, so the file is an input and not a fixture of
//! our own.
//!
//! The split is by dense row, which is the model's own node order. Line 0 is the first
//! `n - K * BATCH` nodes with the edges internal to them ([`prefix`]); batch `k` is the next
//! `BATCH` nodes and every edge whose *later* endpoint is one of them, so an edge that spans
//! two batches is carried by the batch its later endpoint arrives in and never twice.
//!
//! Caveat: this clones records into per-line vectors and writes them as it goes, so peak memory
//! is the whole model plus one batch, and the file is JSON — at 1M nodes and
//! `REFERENCE_DEGREE = 8` it is over a gigabyte. The writer is untimed by construction: it is
//! the input, and nothing in the report is measured across it.

use super::prefix::prefix;
use crate::bench::scale::scale_model;
use crate::bench::tick::Plan;
use graph_core::{EdgeRecord, NodeRecord, REFERENCE_DEGREE};
use graph_wasm::ingest_document;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// The per-line table's header.
const HEADER: &str = "| line | nodes | edges | bytes |\n|---:|---:|---:|---:|";

/// Write the stream `plan` describes to `path` and print what it holds. What can fail is named
/// on the way out — a missing directory, an unwritable file, or a document `ingest_document`
/// refuses to spell because a value is not finite.
pub fn write(plan: &Plan, batch: u32, path: &Path) -> Result<String, String> {
    if batch == 0 {
        return Err("a batch of 0 nodes never grows the graph".into());
    }
    let batches = plan.batches;
    let last = batch
        .checked_mul(batches)
        .ok_or_else(|| format!("batch {batch} x {batches} batches overflows the node count"))?;
    let head = plan.n.checked_sub(last).ok_or_else(|| {
        format!(
            "n={}, batch={batch}, batches={batches}: line 0 needs a node left behind, so \
             batch x batches must be below n",
            plan.n
        )
    })?;
    let (nodes, edges) = scale_model(plan.seed, plan.n, REFERENCE_DEGREE);
    let model = Model::new(&nodes, &edges);
    let mut out = Out::create(path)?;
    let (head_nodes, head_edges) = model.head(head);
    out.line(0, (&head_nodes, &head_edges))?;
    for k in 0..batches {
        let start = head + k * batch;
        let (batch_nodes, batch_edges) = model.batch(start, batch);
        out.line(k + 1, (&batch_nodes, &batch_edges))?;
    }
    let written = out.finish()?;
    Ok(written.table())
}

/// The scale model, sliced by dense row: the whole thing plus the index its edges resolve
/// against. One owner, so the slices and the index cannot disagree about what a row is.
pub(super) struct Model<'a> {
    nodes: &'a [NodeRecord],
    edges: &'a [EdgeRecord],
    /// Node id to dense row, which is the node's position in `nodes`.
    rows: HashMap<&'a str, u32>,
}

impl<'a> Model<'a> {
    pub(super) fn new(nodes: &'a [NodeRecord], edges: &'a [EdgeRecord]) -> Model<'a> {
        let rows = nodes
            .iter()
            .enumerate()
            .map(|(row, n)| (n.id.as_str(), row as u32))
            .collect();
        Model { nodes, edges, rows }
    }

    /// Line 0: the first `keep` rows and the edges internal to them.
    fn head(&self, keep: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
        prefix(self.nodes, self.edges, keep)
    }

    /// Batch `k`: rows `start..start + len`, and every edge whose later endpoint is one of them.
    ///
    /// Caveat: an endpoint is looked up, not searched, so an edge naming a node this model does
    /// not hold panics rather than vanishing. That cannot happen here — `rows` is built from the
    /// same vector the edges are filtered against — and a silent drop would be the worse
    /// failure: the stream would still replay, one edge short, with no refusal.
    pub(super) fn batch(&self, start: u32, len: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
        let nodes = self.nodes[start as usize..(start + len) as usize].to_vec();
        let edges = self
            .edges
            .iter()
            .filter(|e| {
                let later = self.rows[e.source.as_str()].max(self.rows[e.target.as_str()]);
                (start..start + len).contains(&later)
            })
            .cloned()
            .collect();
        (nodes, edges)
    }
}

/// The file being written, and what each line in it held.
struct Out<'a> {
    path: &'a Path,
    file: BufWriter<File>,
    rows: Vec<String>,
    bytes: u64,
}

impl<'a> Out<'a> {
    fn create(path: &'a Path) -> Result<Out<'a>, String> {
        let file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Out {
            path,
            file: BufWriter::new(file),
            rows: Vec::new(),
            bytes: 0,
        })
    }

    /// One line: `graph_wasm::ingest_document`'s text and a newline, and the row it earned.
    fn line(&mut self, line_no: u32, batch: (&[NodeRecord], &[EdgeRecord])) -> Result<(), String> {
        let text = ingest_document(batch.0, batch.1)
            .ok_or_else(|| format!("line {line_no}: a non-finite value JSON cannot spell"))?;
        self.file
            .write_all(text.as_bytes())
            .and_then(|()| self.file.write_all(b"\n"))
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        self.bytes += text.len() as u64 + 1;
        self.rows.push(format!(
            "| {line_no} | {} | {} | {} |",
            batch.0.len(),
            batch.1.len(),
            text.len() + 1
        ));
        Ok(())
    }

    fn finish(mut self) -> Result<Written, String> {
        self.file
            .flush()
            .map_err(|e| format!("{}: {e}", self.path.display()))?;
        Ok(Written {
            lines: self.rows.len(),
            bytes: self.bytes,
            rows: self.rows.join("\n"),
        })
    }
}

/// What the file holds, once it is closed.
struct Written {
    lines: usize,
    bytes: u64,
    rows: String,
}

impl Written {
    /// The table and the one-line summary `--emit` prints on standard output.
    fn table(&self) -> String {
        format!(
            "{HEADER}\n{}\nstream: {} lines, {} bytes",
            self.rows, self.lines, self.bytes
        )
    }
}
