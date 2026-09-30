//! The ANALYSIS and POST stages the gate hashes, one stage's bytes each, and the ids
//! they go by. Split out of `stages.rs` by the house's 300-line limit.
//!
//! **One byte path per stage, shared with the wasm arm — no second serialiser here.**
//! An analysis stage's bytes are `graph_wasm::analysis::to_json`, the same call
//! `gm_analysis_run` delegates to through `stage_exports::analysis_run`, so the two arms
//! compare *that* implementation's output rather than two writers of the same face
//! agreeing by luck. A POST stage's bytes are `graph_wasm::post::snapshot`, the same call
//! `gm_post_run` delegates to through `stage_exports::post_run` — and therefore the very
//! snapshot `gm_snapshot_bytes` hands the wasm arm afterwards. A divergence is then
//! wasm32 having computed something different (D1), not graph-cli having spelled a JSON
//! key twice.
//!
//! **The stage ids are the registries' own**, taken from `graph_wasm::analysis::ANALYSES`
//! and `graph_wasm::post::CAPABILITIES` — the tables `gm_analysis_id` and `gm_post_id`
//! scan — so a row added to either registry joins the gate by being registered, with no
//! edit to this file, and the gate cannot hash an id the ABI does not expose.
//!
//! Each lookup below resolves its id by **scanning** the table for that string rather than
//! enumerating, so "the id a stage is printed under and the row whose bytes it hashes are
//! one row" is true by construction: a row reordered or a new row inserted moves the index
//! and the id together, and no index here is a second spelling of a position.

use graph_core::Topology;
use graph_core::layout::Geometry;
use graph_wasm::analysis::{self, ANALYSES};
use graph_wasm::post::{self, CAPABILITIES};

/// Every ANALYSIS stage, in `ANALYSES` order.
pub fn analyses() -> Vec<&'static str> {
    ANALYSES.iter().map(|entry| entry.id).collect()
}

/// Every POST stage, in `CAPABILITIES` order.
pub fn posts() -> Vec<&'static str> {
    CAPABILITIES.iter().map(|entry| entry.id).collect()
}

/// The analysis stage `id`'s bytes over `topology`: the framed canonical JSON the ABI
/// would return for that analysis, without the frame. The wasm arm reads the same text out
/// of `gm_analysis_run`'s own pointer.
pub fn analysis_bytes(id: &str, topology: &Topology) -> Result<Vec<u8>, String> {
    let i = analysis_index(id).ok_or_else(|| unregistered(id))?;
    let json = analysis::to_json(i, topology).ok_or_else(|| unregistered(id))?;
    Ok(json.into_bytes())
}

/// The POST stage `id`'s bytes over `geometry` on `topology`: the snapshot that pass
/// replaces the handle's with, which is what the ABI then serves.
pub fn post_bytes(id: &str, topology: &Topology, geometry: &Geometry) -> Result<Vec<u8>, String> {
    let i = post_index(id).ok_or_else(|| unregistered(id))?;
    let ran = post::snapshot(i, topology, geometry).ok_or_else(|| unregistered(id))?;
    ran.map(|snapshot| snapshot.to_bytes())
        .map_err(|e| format!("{id}: {e}"))
}

fn analysis_index(id: &str) -> Option<u32> {
    ANALYSES
        .iter()
        .position(|entry| entry.id == id)
        .map(|i| i as u32)
}

fn post_index(id: &str) -> Option<u32> {
    CAPABILITIES
        .iter()
        .position(|entry| entry.id == id)
        .map(|i| i as u32)
}

fn unregistered(id: &str) -> String {
    format!("{id} is not a registered stage, so it has no bytes to hash")
}
