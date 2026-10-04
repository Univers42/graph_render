//! `gm_graph_extend_columns` driven natively (C21), as far as a 64-bit host allows: the
//! export's own checks run here, and the batch goes in one step below it, through
//! [`extend_columns`] — the same split [`super::tests`] uses for `gm_graph_extend`, because a
//! native host has no live `gm_alloc` address for the buffer read.
//!
//! Two things are pinned here that only the export can show: each reader refuses the other
//! format's bytes **on the code** (condition 2 of `docs/decisions/extend-columns.md`), and every
//! graph fault is `ColumnsInvalid` where `gm_graph_extend` publishes `IngestInvalid` for the
//! same fault — the asymmetry the ABI documents rather than hides.

use super::super::{extend_columns, gm_graph_extend_columns};
use super::*;
use crate::columns_batch;
use crate::ingest::MAX_INGEST_BYTES;
use crate::ingest::columns;
use crate::service;
use graph_contract::ingest_columns::Format;

/// A `GMX1` batch for `records`, as `gm_graph_extend_columns` is handed one.
fn batch(records: &Records) -> Vec<u8> {
    columns_batch(&records.0, &records.1)
}

/// The same **nodes** as a `GMC1` document: the same sections with the document's magic word in
/// header word 0, and no edges — because a batch's endpoint cells are entries naming node ids,
/// which the document's dense-row rule refuses outright. That refusal *is* the difference the
/// two formats exist to keep, so the document half of each pair carries no edges.
fn as_document(records: &Records) -> Vec<u8> {
    let mut bytes = columns_batch(&records.0, &[]);
    bytes[0..4].copy_from_slice(&Format::Document.magic().to_le_bytes());
    bytes
}

/// A valid batch with one header word changed: the version, or a reserved word.
fn patched(word: usize, value: u32) -> Vec<u8> {
    let (_, records) = split(23, 12, 8);
    let mut bytes = batch(&records);
    bytes[4 * word..4 * word + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

/// Both directions of "each reader refuses the other's bytes", asserted on the code and not on
/// `is_err`: a `GMX1` batch is `ColumnsInvalid` to the document reader, a `GMC1` document is
/// `ColumnsInvalid` to the batch export, and each reader still takes its own.
#[test]
fn each_reader_refuses_the_other_formats_bytes_on_the_code() {
    let (base, batch_records) = split(21, 20, 10);
    let counts_of = |built: Result<Topology, crate::ingest::columns::ColumnsError>| {
        built.map(|t| (t.node_count(), t.edge_count()))
    };
    let document = as_document(&base);
    let nodes = base.0.len() as u32;
    assert_eq!(counts_of(columns::index(&document)), Ok((nodes, 0)));
    assert_eq!(
        counts_of(columns::index(&batch(&batch_records)))
            .err()
            .map(|e| e.code()),
        Some(Code::ColumnsInvalid),
        "a GMX1 batch is not a document"
    );
    let graph = insert(indexed(&base));
    let before = counts(graph);
    assert_eq!(
        extend_columns(graph, &document),
        Err(Code::ColumnsInvalid),
        "a GMC1 document is not a batch"
    );
    assert_eq!(counts(graph), before, "and nothing was appended");
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
    // A non-finite float is reachable from the records alone — the writer copies the bits, so
    // the decoder's `finite` rule is what refuses it. The cell rules with no record-level
    // spelling (a boolean `2`, a `u32::MAX` in a required column, a nonzero pad) need byte
    // surgery and stay pinned in `ingest_columns/tests/`, one test each.
    let (mut nan, mut infinite) = (batch_records.clone(), batch_records.clone());
    nan.0[0].weight = f64::NAN;
    infinite.1[0].strength = f64::INFINITY;
    let refusals = [
        (batch(&taken), "a node id the graph already holds"),
        (batch(&twice), "an edge id twice in the batch"),
        (batch(&dangling), "an endpoint that names no node"),
        (truncated, "a buffer one byte short"),
        (patched(1, 2), "a version the contract does not name"),
        (patched(6, 1), "a nonzero reserved word"),
        (batch(&nan), "a NaN weight"),
        (batch(&infinite), "an infinite strength"),
        (b"{".to_vec(), "JSON is not a batch"),
    ];
    for (refused, why) in refusals {
        assert_eq!(
            extend_columns(graph, &refused),
            Err(Code::ColumnsInvalid),
            "{why}"
        );
        assert_eq!(counts(graph), before, "{why}");
    }
    assert_eq!(
        extend_columns(graph, &batch(&batch_records)),
        Ok(()),
        "the refusals claimed nothing"
    );
    assert_eq!(counts(graph).0, 30);
}

/// The ceiling `gm_build` holds a document to holds a batch too, by its own code: the length is
/// refused before a byte of it is decoded.
#[test]
fn a_batch_past_the_ingest_ceiling_is_refused_as_too_large() {
    let (base, _) = split(24, 10, 10);
    let graph = insert(indexed(&base));
    let before = counts(graph);
    let over = vec![b' '; MAX_INGEST_BYTES + 1];
    assert_eq!(extend_columns(graph, &over), Err(Code::IngestTooLarge));
    assert_eq!(counts(graph), before);
}

/// The two exports give **different codes for the same logical refusal**: a dangling endpoint
/// is `IngestInvalid` under `gm_graph_extend` and `ColumnsInvalid` here, and the SDK maps them
/// to different error classes. The contract states that asymmetry; this pins it.
#[test]
fn the_same_fault_is_ingest_invalid_through_json_and_columns_invalid_through_columns() {
    let (base, batch_records) = split(25, 30, 20);
    let mut ghost = batch_records.clone();
    let mut edge = ghost.1[0].clone();
    edge.target = "no such node".to_owned();
    ghost.1.push(edge);
    let through_json = insert(indexed(&base));
    let through_columns = insert(indexed(&base));
    let text = crate::seed_ingest::document(&ghost.0, &ghost.1).expect("finite");
    assert_eq!(
        extend(through_json, text.as_bytes()),
        Err(Code::IngestInvalid)
    );
    assert_eq!(
        extend_columns(through_columns, &batch(&ghost)),
        Err(Code::ColumnsInvalid)
    );
    assert_eq!(counts(through_json), counts(through_columns));
}

/// The export's own checks, before the buffer: a dead graph is `InvalidHandle` whatever the
/// buffer, and a live graph with a buffer that is no live allocation is `BuildSourceInvalid`.
#[test]
fn the_columns_export_refuses_a_dead_graph_before_it_looks_at_the_buffer() {
    let (base, batch_records) = split(26, 12, 8);
    let graph = insert(indexed(&base));
    refused_with(
        gm_graph_extend_columns(graph, 0, 0),
        Code::BuildSourceInvalid,
    );
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
    assert_ne!(
        gm_geometry_kind(graph),
        u32::MAX,
        "a refused batch kept the run"
    );
    assert_eq!(extend_columns(graph, &batch(&batch_records)), Ok(()));
    assert_eq!(
        gm_geometry_kind(graph),
        u32::MAX,
        "the run was over the old graph"
    );
}

/// The service façade both exports and the native callers share: over the same records, the
/// JSON path and the columns path end at the same graph.
#[test]
fn the_service_path_appends_what_the_record_path_appends() {
    let (base, batch_records) = split(28, 40, 25);
    let text = crate::seed_ingest::document(&batch_records.0, &batch_records.1).expect("finite");
    let (nodes, edges) = crate::ingest::read_records(text.as_bytes()).expect("the fixture reads");
    let mut json = indexed(&base);
    let mut columns = indexed(&base);
    service::extend(&mut json, text.as_bytes()).expect("a strict batch");
    service::extend_columns(&mut columns, &batch(&batch_records)).expect("a strict batch");
    assert_eq!(json.stats(), columns.stats());
    for n in &nodes {
        assert_eq!(
            json.node_index(&n.id),
            columns.node_index(&n.id),
            "{}",
            n.id
        );
    }
    for e in &edges {
        assert_eq!(
            json.edge_index(&e.id),
            columns.edge_index(&e.id),
            "{}",
            e.id
        );
    }
}
