//! `graph-cli tick --stream <BATCH> --batches <K> --emit <path.jsonl>` writes the input
//! `docs/measurements/perf-p4-delta.md` is measured on, and
//! `graph-cli tick --stream <BATCH> --from <path.jsonl> --layout <engine>` replays it. One
//! input, two arms: the same JSON v1 lines the wasm arm drives through the JS SDK, replayed
//! here through the export's own body ([`graph_wasm::service`]).
//!
//! The stream is line 0 — the whole graph minus the `K * BATCH` nodes that arrive later, with
//! only the edges whose both endpoints are in it — then `K` lines, each the next `BATCH` nodes
//! and every edge whose *later* endpoint is among them. "Later" is the dense row
//! `index_model` would give the node, which is the model's own order, so every node lands in
//! exactly one line and every edge in exactly one batch. Every line is written by
//! `graph_wasm::ingest_document`, the same encoder `gm_build` reads.
//!
//! Two emits of the same plan are byte-identical (a test at `n = 2000`), so a native number
//! and a wasm number are timed over the same bytes rather than over two samples of the same
//! generator.
//!
//! Caveat: the file is as big as the graph in JSON, not in columns — at 1M nodes with
//! `REFERENCE_DEGREE = 8` it is over a gigabyte, so `--emit` and `--from` are minutes of
//! disk, not a step to fold into the timed path. It goes under `target/bench/` and is never
//! committed. Nothing here is part of a layout: `grow.rs`'s `--grow` still carries a session
//! across a fresh index, which is a different question.

pub mod arm;
pub mod emit;
pub mod prefix;
mod stats;

#[cfg(test)]
mod tests;

use super::Plan;
use std::process::ExitCode;

/// Exit 0 with the table on standard output, or 2 when the stream could not be written or read.
pub fn report(plan: &Plan) -> ExitCode {
    match run(plan) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("stream: could not run: {e}");
            ExitCode::from(2)
        }
    }
}

/// Write the stream (`--emit`), or replay it (`--from`). `--stream` with neither is a refusal
/// naming both, rather than a silent no-op that looks like a run.
fn run(plan: &Plan) -> Result<String, String> {
    let Some(batch) = plan.stream else {
        return Err("no --stream <BATCH>: there is no stream to write or to replay".into());
    };
    if let Some(path) = plan.emit.as_deref() {
        return emit::write(plan, batch, path);
    }
    let from = plan.from.as_deref().ok_or_else(|| {
        "--stream wants --emit <path.jsonl> to write the stream, or --from <path.jsonl> to replay one"
            .to_string()
    })?;
    arm::report(plan, batch, from)
}
