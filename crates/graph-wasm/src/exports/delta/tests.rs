//! The two growth exports, driven natively (C21). A 64-bit host has no live `gm_alloc`
//! address, so `gm_graph_extend`'s buffer read cannot run here: the batch goes in one step
//! below the export, through `extend`, and the export itself is driven up to the refusals
//! it makes before the read. Everything else is the exported functions themselves.

use super::super::build::{gm_geometry_kind, gm_node_count, gm_run, insert};
use super::super::columns::gm_release;
use super::super::session::gm_force_session_tick;
use super::super::session::{gm_force_session_create, gm_force_session_release};
use super::super::state::HANDLES;
use super::{extend, gm_force_session_grow, gm_graph_extend};
use crate::errors::{self, Code};
use crate::ingest::MAX_INGEST_BYTES;
use crate::seed_ingest::document;
use graph_core::layout::force::{ForceSession, LiveParams};
use graph_core::registry::LAYOUTS;
use graph_core::{EdgeRecord, NodeRecord, REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::collections::BTreeSet;

type Records = (Vec<NodeRecord>, Vec<EdgeRecord>);

mod columns;

/// The gate's model for `seed` cut after node `kept`: the base holds the first `kept` nodes and
/// every edge between them, the batch the rest, both in model order.
fn split(seed: u32, count: u32, kept: usize) -> (Records, Records) {
    let (mut nodes, edges) = seeded_model(seed, count, REFERENCE_DEGREE);
    let batch_nodes = nodes.split_off(kept);
    let old: BTreeSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let (base_edges, batch_edges) = edges
        .into_iter()
        .partition(|e| old.contains(&e.source) && old.contains(&e.target));
    ((nodes, base_edges), (batch_nodes, batch_edges))
}

fn indexed((nodes, edges): &Records) -> Topology {
    index_model(nodes, edges).expect("the model fits")
}

fn bytes((nodes, edges): &Records) -> Vec<u8> {
    document(nodes, edges).expect("finite").into_bytes()
}

fn joined(base: &Records, batch: &Records) -> Records {
    let nodes = [base.0.clone(), batch.0.clone()].concat();
    (nodes, [base.1.clone(), batch.1.clone()].concat())
}

fn bits_of(session: &ForceSession) -> (Vec<u64>, Vec<u64>) {
    let column = |values: &[f64]| values.iter().map(|v| v.to_bits()).collect();
    (column(session.xs()), column(session.ys()))
}

fn session_bits(id: u32) -> (Vec<u64>, Vec<u64>) {
    crate::session::with(id, |session| Ok(bits_of(session))).expect("a live session")
}

fn live_session(graph: u32) -> u32 {
    let id = gm_force_session_create(graph, 0, 0);
    assert_ne!(id, 0, "create refused with {}", errors::get());
    id
}

fn refused_with(word: u32, code: Code) {
    assert_eq!((word, errors::get()), (0, code as u32));
}

/// Step 1's equivalence: extend then grow lands on the bits a session created over the base
/// and carried onto the whole graph lands on, ticks before and after the batch included.
#[test]
fn extend_then_grow_is_build_all_then_create_then_carry() {
    let (base, batch) = split(7, 60, 40);
    let graph = insert(indexed(&base));
    let id = live_session(graph);
    assert_ne!(gm_force_session_tick(id, 10), 0);
    assert_eq!(extend(graph, &bytes(&batch)), Ok(()));
    assert_eq!(gm_force_session_grow(id, graph), 1);
    assert_ne!(gm_force_session_tick(id, 10), 0);

    let (before, after) = (indexed(&base), indexed(&joined(&base, &batch)));
    let mut direct = ForceSession::new(&before, LiveParams::default()).expect("in range");
    direct.step(10);
    let mut carried = direct.carry(&before, &after).expect("carries");
    carried.step(10);
    assert_eq!(gm_node_count(graph), 60);
    assert_eq!(session_bits(id), bits_of(&carried));
}

/// Every batch refusal leaves the graph as it was: the counts, and a node id the refused
/// batch named, which a later batch can still claim.
#[test]
fn a_refused_batch_leaves_the_graph_unchanged() {
    let (base, batch) = split(8, 30, 20);
    let graph = insert(indexed(&base));
    let before = counts(graph);
    let (mut taken, mut dangling) = (batch.clone(), batch.clone());
    taken.0.push(base.0[0].clone());
    let mut edge = batch.1.first().cloned().expect("the batch has edges");
    edge.target = "no such node".to_owned();
    dangling.1.push(edge);
    let refusals = [
        (b"{".to_vec(), "bad JSON"),
        (bytes(&taken), "a taken id"),
        (bytes(&dangling), "a dangling endpoint"),
    ];
    for (refused, why) in refusals {
        assert_eq!(extend(graph, &refused), Err(Code::IngestInvalid), "{why}");
        assert_eq!(counts(graph), before, "{why}");
    }
    assert_eq!(
        extend(graph, &bytes(&batch)),
        Ok(()),
        "the refused ids were never claimed"
    );
    assert_eq!(gm_node_count(graph), 30);
}

/// A graph's node and edge counts, read off the handle table.
fn counts(graph: u32) -> (u32, u32) {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let topology = &handles.get(graph).expect("a live graph").topology;
        (topology.node_count(), topology.edge_count())
    })
}

/// The ceiling `gm_build` holds a document to holds a batch too, by its own code.
#[test]
fn a_batch_past_the_ingest_ceiling_is_refused_as_too_large() {
    let (base, _) = split(9, 10, 10);
    let graph = insert(indexed(&base));
    let over = vec![b' '; MAX_INGEST_BYTES + 1];
    assert_eq!(extend(graph, &over), Err(Code::IngestTooLarge));
    assert_eq!(gm_node_count(graph), 10);
}

/// The export's own checks, before the buffer: a dead graph is `InvalidHandle` whatever the
/// buffer, and a live graph with a buffer that is no live allocation is `BuildSourceInvalid`.
#[test]
fn the_export_refuses_a_dead_graph_before_it_looks_at_the_buffer() {
    let (base, batch) = split(10, 12, 8);
    let graph = insert(indexed(&base));
    refused_with(gm_graph_extend(graph, 0, 0), Code::BuildSourceInvalid);
    gm_release(graph);
    refused_with(gm_graph_extend(graph, 0, 0), Code::InvalidHandle);
    assert_eq!(extend(graph, &bytes(&batch)), Err(Code::InvalidHandle));
}

/// A batch voids the last run (C4), as a failed `gm_run` does; a refused one keeps it.
#[test]
fn a_batch_clears_the_last_run_and_a_refusal_keeps_it() {
    let (base, batch) = split(11, 16, 10);
    let graph = insert(indexed(&base));
    let grid = LAYOUTS
        .iter()
        .position(|l| l.id == "layout.grid")
        .expect("grid");
    assert_eq!(gm_run(graph, u32::try_from(grid).unwrap(), 0, 0), 1);
    assert_eq!(extend(graph, b"{"), Err(Code::IngestInvalid));
    assert_ne!(
        gm_geometry_kind(graph),
        u32::MAX,
        "a refused batch kept the run"
    );
    assert_eq!(extend(graph, &bytes(&batch)), Ok(()));
    assert_eq!(
        gm_geometry_kind(graph),
        u32::MAX,
        "the run was over the old graph"
    );
}

#[test]
fn grow_refuses_a_session_that_is_not_live() {
    let (base, _) = split(12, 10, 10);
    let graph = insert(indexed(&base));
    let id = live_session(graph);
    assert_eq!(gm_force_session_release(id), 1);
    refused_with(gm_force_session_grow(id, graph), Code::InvalidSession);
}

#[test]
fn grow_refuses_a_released_graph_with_invalid_handle() {
    let (base, _) = split(13, 10, 10);
    let graph = insert(indexed(&base));
    let id = live_session(graph);
    gm_release(graph);
    refused_with(gm_force_session_grow(id, graph), Code::InvalidHandle);
}

/// A second graph is refused even when it is a larger graph `ForceSession::grow` would
/// accept, and the session is left as it was.
#[test]
fn grow_against_a_second_graph_is_refused_and_changes_nothing() {
    let (base, batch) = split(14, 30, 20);
    let graph = insert(indexed(&base));
    let other = insert(indexed(&joined(&base, &batch)));
    let id = live_session(graph);
    let before = session_bits(id);
    refused_with(gm_force_session_grow(id, other), Code::SessionRefused);
    assert_eq!(session_bits(id), before);
}

/// A grow with nothing appended is a success that moves nothing.
#[test]
fn grow_with_nothing_appended_changes_nothing() {
    let (base, _) = split(15, 20, 20);
    let graph = insert(indexed(&base));
    let id = live_session(graph);
    assert_ne!(gm_force_session_tick(id, 3), 0);
    let before = session_bits(id);
    assert_eq!(gm_force_session_grow(id, graph), 1);
    assert_eq!(errors::get(), 0);
    assert_eq!(session_bits(id), before);
}
