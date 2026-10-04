//! The stream's own claims, at a size that runs in a test: that two emits of one plan are
//! byte-identical, that the batches partition the model rather than overlapping it, and that
//! a stream nobody wrote is a refusal and not an empty table.
//!
//! Caveat: `n = 2000` with batches of 100 is small enough that the dense-row split is trivial
//! and large enough that an edge spanning a batch boundary appears — the `partitions` test
//! fails if the "later endpoint" rule ever becomes the "either endpoint" one, which at
//! `n = 2000` it can.

use super::arm;
use super::emit;
use super::prefix;
use crate::bench::tick::{Layout, Plan};
use graph_core::{REFERENCE_DEGREE, seeded_model};
use std::path::PathBuf;

const N: u32 = 2_000;
const BATCH: u32 = 100;
const BATCHES: u32 = 3;

fn plan() -> Plan {
    Plan {
        layout: Layout::BarnesHut,
        n: N,
        ticks: 10,
        warm: 2,
        seed: 0,
        workers: 1,
        grow: None,
        collide_radius: None,
        passes: false,
        stream: Some(BATCH),
        batches: BATCHES,
        emit: None,
        from: None,
    }
}

/// A path under the system temp directory, named for this process and the test, so a parallel
/// `cargo test` run never reads another run's file.
fn path(tag: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("gm-stream-{tag}-{}.jsonl", std::process::id()));
    path
}

fn write(tag: &str) -> String {
    let path = path(tag);
    let text = emit::write(&plan(), BATCH, &path).expect("the stream should be writable");
    let read = std::fs::read_to_string(&path).expect("the stream should be readable back");
    assert!(text.contains(&format!("{} lines", BATCHES as usize + 1)));
    assert!(read.ends_with('\n'));
    read
}

/// Two emits of one plan are byte-identical, so the native and the wasm arm are timed over the
/// same bytes rather than over two samples of the same generator.
#[test]
fn two_emits_are_byte_identical() {
    assert_eq!(write("identical-a"), write("identical-b"));
}

/// Line 0 is the head, then one line per batch, and every node and every edge of the model
/// lands in exactly one of them.
#[test]
fn the_batches_partition_the_model() {
    let (nodes, edges) = seeded_model(0, N, REFERENCE_DEGREE);
    let head = prefix::prefix(&nodes, &edges, N - BATCH * BATCHES);
    assert_eq!(head.0.len() as u32, N - BATCH * BATCHES);
    let mut seen_nodes: Vec<String> = head.0.iter().map(|n| n.id.clone()).collect();
    let mut seen_edges: Vec<String> = head.1.iter().map(|e| e.id.clone()).collect();
    let mut rows: std::collections::HashMap<String, u32> = head
        .0
        .iter()
        .enumerate()
        .map(|(row, n)| (n.id.clone(), row as u32))
        .collect();
    for k in 0..BATCHES {
        let start = N - BATCH * BATCHES + k * BATCH;
        let model = emit::Model::new(&nodes, &edges);
        let (batch_nodes, batch_edges) = model.batch(start, BATCH);
        assert_eq!(
            batch_nodes.len() as u32,
            BATCH,
            "batch {k} holds a whole batch"
        );
        assert!(
            !batch_edges.is_empty(),
            "batch {k} holds no edge, so the boundary case is never reached"
        );
        for node in &batch_nodes {
            assert!(rows.insert(node.id.clone(), start).is_none(), "node twice");
            seen_nodes.push(node.id.clone());
        }
        for edge in &batch_edges {
            let later = rows[&edge.source].max(rows[&edge.target]);
            assert!(
                (start..start + BATCH).contains(&later),
                "edge in the wrong batch"
            );
            seen_edges.push(edge.id.clone());
        }
    }
    let mut all_nodes: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let mut all_edges: Vec<String> = edges.iter().map(|e| e.id.clone()).collect();
    seen_nodes.sort();
    seen_edges.sort();
    all_nodes.sort();
    all_edges.sort();
    assert_eq!(
        seen_nodes, all_nodes,
        "the stream lost or duplicated a node"
    );
    assert_eq!(
        seen_edges, all_edges,
        "the stream lost or duplicated an edge"
    );
}

/// A stream nobody wrote is a refusal, not a table with no rows.
#[test]
fn a_missing_stream_is_a_refusal() {
    let missing = PathBuf::from("/nonexistent/gm-stream/never-written.jsonl");
    let refused = arm::report(&plan(), BATCH, &missing);
    assert!(refused.is_err(), "a missing file should not report a run");
}

/// `--stream` with neither `--emit` nor `--from` says what it wanted.
#[test]
fn a_stream_with_no_endpoint_is_a_refusal() {
    assert!(super::run(&plan()).is_err());
}

/// `prefix` refuses to be a second copy: the edge count it keeps is the model's own, checked
/// against an independent walk.
#[test]
fn the_head_keeps_internal_edges_only() {
    let (nodes, edges) = seeded_model(0, N, REFERENCE_DEGREE);
    let keep = N - BATCH * BATCHES;
    let (kept_nodes, kept_edges) = prefix::prefix(&nodes, &edges, keep);
    let ids: std::collections::HashSet<&str> = kept_nodes.iter().map(|n| n.id.as_str()).collect();
    let expected = edges
        .iter()
        .filter(|e| ids.contains(e.source.as_str()) && ids.contains(e.target.as_str()))
        .count();
    assert_eq!(kept_edges.len(), expected);
    assert!(
        kept_edges.len() < edges.len(),
        "the head must drop the open edges"
    );
}
