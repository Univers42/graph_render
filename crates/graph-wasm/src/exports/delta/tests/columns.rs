//! `gm_graph_extend_columns` driven natively (C21), as far as a 64-bit host allows: the
//! export's own checks run here, and the batch goes in one step below it, through
//! [`extend_columns`] — the same split [`super::tests`] uses for `gm_graph_extend`, because a
//! native host has no live `gm_alloc` address for the buffer read.
//!
//! Two things are pinned here that only the export can show: each reader refuses the other
//! format's bytes **on the code** (condition 2), and every graph fault is `ColumnsInvalid`
//! while `gm_graph_extend` publishes `IngestInvalid` for the same fault — the asymmetry the
//! ABI documents rather than hides.

use super::*;
use crate::ingest::MAX_INGEST_BYTES;
use crate::ingest::columns;
use crate::{columns_batch, service};
use graph_contract::ingest_columns::Format;
use graph_core::{index_model, seeded_model, REFERENCE_DEGREE};

type Records = (Vec<NodeRecord>, Vec<EdgeRecord>);

/// The gate's model for `seed` cut after node `kept`, exactly as [`super::tests::split`].
fn split(seed: u32, count: u32, kept: usize) -> (Records, Records) {
    let (mut nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
    let batch_nodes = nodes.split_off(kept);
    let old: std::collections::BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let (base_edges, batch_edges) = edges
        .into_iter()
        .partition(|e| old.contains(&e.source) && old.contains(&e.target));
    ((nodes, base_edges), (batch_nodes, batch_edges))
}

fn indexed((nodes, edges): &Records) -> Topology {
    index_model(nodes, edges).expect("the model fits")
}

fn counts(graph: u32) -> (u32, u32) {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let topology = &handles.get(graph).expect("a live graph").topology;
        (topology.node_count(), topology.edge_count())
    })
}

/// A `GMX1` batch for `records`, as `gm_graph_extend_columns` is handed one.
fn batch(records: &Records) -> Vec<u8> {
    columns_batch(&records.0, &records.1)
}

/// The same records as a **document**: the same sections, `GMC1`'s magic word in header word 0.
/// Nothing here is a document's endpoint, so the word is the only difference — which is the
/// whole claim these two tests make.
fn as_document(records: &Records) -> Vec<u8> {
    let mut bytes = columns_batch(&records.0, &records.1);
    bytes[0..4].copy_from_slice(&Format::Document.magic().to_le_bytes());
    bytes
}

/// Both directions of "each reader refuses the other's bytes", asserted on the code rather
/// than on `is_err`: a batch is `ColumnsInvalid` to `gm_build_columns`, a document is
/// `ColumnsInvalid` to `gm_graph_extend_columns`, and each reader takes its own.
#[test]
fn each_reader_refuses_the_other_formats_bytes_on_the_code() {
    let (base, batch_records) = split(21, 20, 10);
    let mut expected = (base.0.len() as u32, base.1.len() as u32);
    let document = as_document(&base);
    expected = columns::index(&document)
        .map(|t| (t.node_count(), t.edge_count()))
        .expect("the same records as a document still index");
    assert_eq!(expected, (base.0.len() as u32, base.1.len() as u32));

    assert_eq!(
        columns::index(&batch(&batch_records)).map_err(|e| e.code()),
        Err(Code::ColumnsInvalid),
        "a GMX1 batch is not a document"
    );
    let graph = insert(indexed(&base));
    assert_eq!(
        extend_columns(graph, &document),
        Err(Code::ColumnsInvalid),
        "a GMC1 document is not a batch"
    );
    assert_eq!(counts(graph), expected, "and nothing was appended");
}

/// One row per refusal the export can reach: a header word, the exact length, and the four
/// graph faults. Each is `ColumnsInvalid`, and each leaves the graph exactly as it was.
#[test]
fn a_refused_batch_leaves_the_graph_unchanged() {
    let (base, batch_records) = split(22, 30, 20);
    let graph = insert(indexed(&base));
    let before = counts(graph);
    let (mut taken, mut twice, mut dangling) = (
        batch_records.clone(),
        batch_records.clone(),
        batch_records.clone(),
    );
    taken.0.push(base.0[0].clone());
    let mut same_id = batch_records.1[0].clone();
    same_id.source = same_id.target.clone();
    twice.1.push(same_id);
    let mut ghost = batch_records.1[0].clone();
    ghost.target = "no such node".to_owned();
    dangling.1.push(ghost);
    let mut truncated = batch(&batch_records);
    truncated.truncate(truncated.len() - 1);
    let refusals = [
        (batch(&taken), "a node id the graph already holds"),
        (batch(&twice), "an edge id twice in the batch"),
        (batch(&dangling), "an endpoint that names no node"),
        (truncated, "a buffer one byte short"),
        (header(2, "a version the contract does not name"), "a bad version"),
        (header(6, "a nonzero reserved word"), "a nonzero reserved word"),
        (b"{".to_vec(), "JSON is not a batch"),
    ];
    for (refused, why) in refusals {
        assert_eq!(extend_columns(graph, &refused), Err(Code::ColumnsInvalid), "{why}");
        assert_eq!(counts(graph), before, "{why}");
    }
    assert_eq!(extend_columns(graph, &batch(&batch_records)), Ok(()), "the refusals claimed nothing");
    assert_eq!(counts(graph).0, 30);
}

/// One header word of a valid batch, changed: the two the contract fixes that a host can reach
/// without knowing the section table.
fn header(word: usize, _why: &str) -> Vec<u8> {
    let (base, batch_records) = split(23, 12, 8);
    let mut bytes = batch(&batch_records);
    let value = if word == 2 { 2 } else { 1 };
    bytes[4 * word..4 * word + 4].copy_from_slice(&value.to_le_bytes());
    let _ = base;
    bytes
}

/// The ceiling `gm_build` holds a document to holds a batch too, by its own code: the length is
/// refused before a byte of it is decoded.
#[test]
fn a_batch_past_the_ingest_ceiling_is_refused_as_too_large() {
    let (base, _) = split(24, 10, 10);
    let graph = insert(indexed(&base));
    let over = vec![b' '; MAX_INGEST_BYTES + 1];
    assert_eq!(extend_columns(graph, &over), Err(Code::IngestTooLarge));
    assert_eq!(counts(graph), (10, 0));
}

/// The two exports give **different codes for the same logical refusal**: a dangling endpoint
/// is `IngestInvalid` under `gm_graph_extend` and `ColumnsInvalid` here. The SDK maps them to
/// different error classes, so this asymmetry is stated in the contract and pinned here.
#[test]
fn the_same_fault_is_ingest_invalid_through_json_and_columns_invalid_through_columns() {
    let (base, batch_records) = split(25, 30, 20);
    let mut ghost = batch_records.clone();
    let mut edge = ghost.1[0].clone();
    edge.target = "no such node".to_owned();
    ghost.1.push(edge);
    let json = insert(indexed(&base));
    let columns_graph = insert(indexed(&base));
    let (json_doc, _) = ghost;
    let text = crate::seed_ingest::document(&json_doc.0, &json_doc.1).expect("finite");
    assert_eq!(extend(json, text.as_bytes()), Err(Code::IngestInvalid));
    assert_eq!(
        extend_columns(columns_graph, &batch(&ghost)),
        Err(Code::ColumnsInvalid)
    );
    assert_eq!(counts(json), counts(columns_graph));
}

/// The export's own checks, before the buffer: a dead graph is `InvalidHandle` whatever the
/// buffer, and a live graph with a buffer that is no live allocation is `BuildSourceInvalid`.
#[test]
fn the_columns_export_refuses_a_dead_graph_before_it_looks_at_the_buffer() {
    let (base, batch_records) = split(26, 12, 8);
    let graph = insert(indexed(&base));
    refused_with(gm_graph_extend_columns(graph, 0, 0), Code::BuildSourceInvalid);
    gm_release(graph);
    refused_with(gm_graph_extend_columns(graph, 0, 0), Code::InvalidHandle);
    assert_eq!(
        extend_columns(graph, &batch(&batch_records)),
        Err(Code::InvalidHandle)
    );
}

/// A batch voids the last run (C4) as `gm_graph_extend` does, through the shared `append`.
#[test]
fn a_columns_batch_clears_the_last_run_and_a_refusal_keeps_it() {
    let (base, batch_records) = split(27, 16, 10);
    let graph = insert(indexed(&base));
    let grid = graph_core::registry::LAYOUTS
        .iter()
        .position(|l| l.id == "layout.grid")
        .expect("grid");
    assert_eq!(gm_run(graph, u32::try_from(grid).unwrap(), 0, 0), 1);
    assert_eq!(extend_columns(graph, b"{"), Err(Code::ColumnsInvalid));
    assert_ne!(gm_geometry_kind(graph), u32::MAX, "a refused batch kept the run");
    assert_eq!(extend_columns(graph, &batch(&batch_records)), Ok(()));
    assert_eq!(gm_geometry_kind(graph), u32::MAX, "the run was over the old graph");
}

/// The service façade is what both exports and the native callers share, so the two paths over
/// the same records must end at the same graph.
#[test]
fn the_service_path_appends_what_the_export_path_appends() {
    let (base, batch_records) = split(28, 40, 25);
    let through_json = insert(indexed(&base));
    let through_columns = insert(indexed(&base));
    let text = crate::seed_ingest::document(&batch_records.0, &batch_records.1).expect("finite");
    assert_eq!(service::extend(through_json_ref(&through_json), text.as_bytes()), Ok(()));
    assert_eq!(
        service::extend_columns(through_columns_ref(&through_columns), &batch(&batch_records)),
        Ok(())
    );
    assert_eq!(counts(through_json), counts(through_columns));
}

/// The graph's topology, borrowed out of the handle table for the one comparison the two
/// paths must agree on.
fn through_json_ref(graph: &u32) -> &mut Topology {
    unreachable!("replaced below")
}