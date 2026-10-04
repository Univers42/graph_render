//! Gate-only: the hash gate's seed model, written as the provisional ingest JSON
//! (`ingest`) reads (C20: "the seed's ingest produced in-module by a documented
//! gate-only export"). This is what lets `harness/wasm-run.mjs`'s hash mode drive a
//! seed through the *real* ABI — `gm_alloc`/`gm_build`/`gm_run`/`gm_snapshot_bytes` —
//! and still land on the same synthetic model the retained `gm_layout_grid` shim hashes,
//! so the two can be asserted equal.
//!
//! Target-independent: pure string building, no wasm pointer. Round-tripped through
//! `crate::ingest::read` (test-only) in this module's own tests, so the two are proven consistent
//! natively, with no wasm build in the loop.

use crate::json_string::push_quoted as string;
use graph_core::{EdgeRecord, NodeRecord};
use std::fmt::Write as _;

/// The provisional ingest JSON for the hash gate's model at `seed`, at the gate's
/// standard node count and reference degree (`graph_core::gate_node_count`,
/// `graph_core::REFERENCE_DEGREE` — the same inputs `gm_topology`/`gm_layout_grid` use).
/// `None` as [`document`] refuses.
pub fn for_seed(seed: u32) -> Option<String> {
    let (nodes, edges) = graph_core::seeded_model(
        seed,
        graph_core::gate_node_count(seed),
        graph_core::REFERENCE_DEGREE,
    );
    document(&nodes, &edges)
}

/// The provisional ingest document for `nodes` and `edges`, in ingest order. `None` when
/// a `weight`, `version` or `strength` is not finite: JSON has no `inf` or `NaN`, and
/// `ingest::read` refuses a document that spells one.
pub fn document(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Option<String> {
    let finite = nodes
        .iter()
        .all(|n| n.weight.is_finite() && n.version.is_finite())
        && edges.iter().all(|e| e.strength.is_finite());
    if !finite {
        return None;
    }
    let mut out = format!(r#"{{"version":{},"nodes":["#, crate::ingest::VERSION);
    for (i, n) in nodes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_node(&mut out, n);
    }
    out.push_str(r#"],"edges":["#);
    for (i, e) in edges.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        push_edge(&mut out, e);
    }
    out.push_str("]}");
    Some(out)
}

fn push_node(out: &mut String, n: &NodeRecord) {
    out.push('{');
    field(out, "id", true);
    string(out, &n.id);
    field(out, "kind", false);
    string(out, n.kind.as_str());
    field(out, "database_id", false);
    opt_string(out, n.database_id.as_deref());
    field(out, "source", false);
    string(out, &n.source);
    field(out, "label", false);
    string(out, &n.label);
    field(out, "group", false);
    opt_string(out, n.group.as_deref());
    field(out, "weight", false);
    number(out, n.weight);
    field(out, "version", false);
    number(out, n.version);
    field(out, "has_note", false);
    boolean(out, n.has_note);
    field(out, "icon", false);
    opt_string(out, n.icon.as_deref());
    out.push('}');
}

fn push_edge(out: &mut String, e: &EdgeRecord) {
    out.push('{');
    field(out, "id", true);
    string(out, &e.id);
    field(out, "source", false);
    string(out, &e.source);
    field(out, "target", false);
    string(out, &e.target);
    field(out, "kind", false);
    string(out, e.kind.as_str());
    field(out, "label", false);
    string(out, &e.label);
    field(out, "strength", false);
    number(out, e.strength);
    field(out, "directed", false);
    boolean(out, e.directed);
    field(out, "record_id", false);
    opt_string(out, e.record_id.as_deref());
    field(out, "child_first", false);
    boolean(out, e.child_first);
    out.push('}');
}

fn field(out: &mut String, name: &str, first: bool) {
    if !first {
        out.push(',');
    }
    out.push('"');
    out.push_str(name);
    out.push_str("\":");
}

fn opt_string(out: &mut String, text: Option<&str>) {
    match text {
        Some(text) => string(out, text),
        None => out.push_str("null"),
    }
}

fn number(out: &mut String, value: f64) {
    let _ = write!(out, "{value}");
}

fn boolean(out: &mut String, value: bool) {
    out.push_str(if value { "true" } else { "false" });
}

#[cfg(test)]
mod tests;
