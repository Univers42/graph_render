//! The two stages downstream of LAYOUT, as the handle table sees them: what a POST pass
//! reads off a handle, and what a caller gets back for running one.
//!
//! Target-independent, so every branch of `gm_post_run` and `gm_analysis_run` is pinned
//! natively without a wasm build (C21): the exports in `exports/stages.rs` are these
//! three functions plus a refusal table, and both refusals each of them can reach — an
//! unknown handle, and an index past the end of a registry — are decided here.

use crate::analysis;
use crate::errors::Code;
use crate::handle::Handles;
use crate::post;
use crate::service;
use graph_contract::binary::Snapshot;

#[cfg(test)]
mod tests;

/// Why a post pass was refused, before it becomes a `Code` on the wire. `Ok(None)` is
/// not a case: a refusal always has a name.
pub type Refusal = Code;

/// The id at `i`, or the refusal a caller sees as `gm_post_id`'s `0`.
pub fn post_id(i: u32) -> Result<&'static str, Refusal> {
    post::id_at(i).ok_or(Code::IndexOutOfRange)
}

/// The id at `i`, or `gm_analysis_id`'s refusal.
pub fn analysis_id(i: u32) -> Result<&'static str, Refusal> {
    analysis::id_at(i).ok_or(Code::IndexOutOfRange)
}

/// Runs POST capability `post_index` over `handle`'s last successful layout run, and
/// returns the snapshot that replaces it — the same handle's new edge geometry, which
/// every column read afterwards sees. The pass reads the layout's geometry in place and
/// the handle keeps the one snapshot it produced: no copy of either per run (F-98).
///
/// A handle with no successful run yet is refused (`NoGeometryYet`): a post pass has
/// nothing to read without a layout's positions, and drawing one anyway would put
/// unattributed points on the wire. A pass that fails, or whose edges do not fit the
/// snapshot, is `PostFailed` — and **the handle keeps the geometry it had**, so a failed
/// pass never leaves a half-applied one behind for the next read to serve.
pub fn post_run(handles: &mut Handles, handle: u32, post_index: u32) -> Result<&Snapshot, Refusal> {
    let Some(entry) = handles.get_mut(handle) else {
        return Err(Code::InvalidHandle);
    };
    let Some(geometry) = entry.geometry.as_ref() else {
        return Err(Code::NoGeometryYet);
    };
    // The snapshot itself is `crate::post::snapshot`, the same call the hash gate's native
    // arm makes: one byte path for both arms, so a POST stage's divergence is about wasm32
    // and not about two writers of the same face (see `hashgate/stages.rs`). Through
    // `service::post_pass`, the call the native service makes too.
    let snapshot = service::post_pass(&entry.topology, geometry, post_index)?;
    Ok(entry.snapshot.insert(snapshot))
}

/// The canonical JSON face of analysis `index` over `handle`'s topology, or its refusal.
///
/// **No geometry is required**: every analysis in `graph_core::analysis` is a function of
/// the topology alone, so a caller may analyse straight after `gm_build` and before any
/// layout has run. The refusals are an unknown handle, an index past the end, and a
/// report with no JSON text (`AnalysisFailed`: a non-finite score, never written as `NaN`).
pub fn analysis_run(handles: &Handles, handle: u32, index: u32) -> Result<String, Refusal> {
    let Some(entry) = handles.get(handle) else {
        return Err(Code::InvalidHandle);
    };
    let report = analysis::run(index, &entry.topology).ok_or(Code::IndexOutOfRange)?;
    report.to_json().ok_or(Code::AnalysisFailed)
}
