//! The native façade: build a topology, run a layout and at most one POST pass over it, and
//! get the snapshot's binary bytes, with no handle table, no linear memory and no
//! thread-local error. The HTTP service links this crate as an rlib and calls only this
//! module, so a request it answers and the same request made through the wasm module are
//! answered by one code path: `gm_build`, `gm_build_contract`, `gm_run` and `gm_post_run`
//! call [`build`], [`snapshot_of`] and [`post_pass`] too (`docs/contract/service-api.md`
//! "Verdict", condition 1).
//!
//! A caller reads the [`Result`]; [`Code`] is the same refusal the export would publish
//! through `gm_last_error`, and [`Code::name`] the name the SDK gives it.

use graph_contract::binary::Snapshot;
use graph_core::registry::{self, LAYOUTS};
use graph_core::{Geometry, StageError, Topology};

pub use crate::errors::Code;

// The probe build lifts the ingest ceiling (`ingest::ceiling`) and writes a process-wide
// mark table without a lock (`ingest/phases.rs`), a data race once two threads build. Only
// graph-cli's wasm32 build turns it on; a native build that does is refused here.
#[cfg(all(feature = "probe", not(target_arch = "wasm32"), not(test)))]
compile_error!(
    "graph-wasm's `probe` feature is wasm32-only: natively it races and lifts the ingest ceiling"
);

/// Which reader [`build`] hands the bytes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The provisional node/edge JSON, as `gm_build` reads it.
    Ingest,
    /// The ingest contract (`docs/contract/ingest-schema.json`), as `gm_build_contract`
    /// reads it.
    Contract,
}

/// The topology `bytes` describe, or the refusal the matching build export publishes:
/// [`Code::IngestInvalid`] or [`Code::IngestTooLarge`] for [`Source::Ingest`],
/// [`Code::ContractInvalid`] for [`Source::Contract`].
pub fn build(bytes: &[u8], source: Source) -> Result<Topology, Code> {
    match source {
        // The records are dropped as soon as the topology holds them.
        Source::Ingest => crate::ingest::read_records(bytes)
            .and_then(|(nodes, edges)| crate::ingest::index(&nodes, &edges))
            .map_err(|refusal| refusal.code()),
        Source::Contract => crate::contract::derive(bytes)
            .map(|(_, topology)| topology)
            .map_err(|_| Code::ContractInvalid),
    }
}

/// Appends the provisional node/edge document `bytes` holds to `topology`: `gm_graph_extend`'s
/// body (`docs/contract/delta.md`). Read by the reader [`build`] uses for [`Source::Ingest`], so
/// a batch is refused for the reasons a whole document is ([`Code::IngestInvalid`],
/// [`Code::IngestTooLarge`]); a batch `Topology::extend` refuses (a repeated id, a dangling
/// endpoint) is [`Code::IngestInvalid`] too. On any refusal `topology` is unchanged.
pub fn extend(topology: &mut Topology, bytes: &[u8]) -> Result<(), Code> {
    let (nodes, edges) = crate::ingest::read_records(bytes).map_err(|refusal| refusal.code())?;
    topology
        .extend(&nodes, &edges)
        .map_err(|_| Code::IngestInvalid)
}

/// The binary snapshot (`docs/contract/binary-layout.md`) of `layout` over `topology`, then
/// of the POST pass `post` over that layout's geometry when one is named: the bytes
/// `gm_snapshot_bytes` reads after `gm_run`, and after `gm_post_run` when `post` is given.
///
/// Refusals: an id no layout is registered under is [`Code::UnknownLayoutId`], an id no
/// POST capability is registered under is [`Code::IndexOutOfRange`] (the refusal
/// `gm_post_run` gives an index past the end), then [`Code::LayoutFailed`] and
/// [`Code::PostFailed`] as the exports give them. Both ids are checked before anything
/// runs, so a request naming an unknown POST id costs no layout.
pub fn run(topology: &Topology, layout: &str, post: Option<&str>) -> Result<Vec<u8>, Code> {
    let layout = registry::find(layout).ok_or(Code::UnknownLayoutId)?;
    let post = post.map(post_index).transpose()?;
    let (geometry, snapshot) = snapshot_of(topology, (layout.run)(topology))?;
    let snapshot = match post {
        Some(index) => post_pass(topology, &geometry, index)?,
        None => snapshot,
    };
    Ok(snapshot.to_bytes())
}

/// Every registered layout id, in registry order (`gm_layout_id`'s order).
pub fn layout_ids() -> impl Iterator<Item = &'static str> {
    LAYOUTS.iter().map(|layout| layout.id)
}

/// Every POST capability id, in registry order (`gm_post_id`'s order).
pub fn post_ids() -> impl Iterator<Item = &'static str> {
    crate::post::CAPABILITIES.iter().map(|entry| entry.id)
}

/// A layout's outcome as the geometry a POST pass reads and the snapshot it is published
/// as; either failure is [`Code::LayoutFailed`]. `gm_run`'s and `gm_run_threaded`'s body.
pub(crate) fn snapshot_of(
    topology: &Topology,
    ran: Result<Geometry, StageError>,
) -> Result<(Geometry, Snapshot), Code> {
    let geometry = ran.map_err(|_| Code::LayoutFailed)?;
    let snapshot =
        graph_core::layout::snapshot(topology, geometry.clone()).map_err(|_| Code::LayoutFailed)?;
    Ok((geometry, snapshot))
}

/// The POST capability at `index` over a layout's `geometry`, as its snapshot:
/// [`Code::IndexOutOfRange`] past the end, [`Code::PostFailed`] when the pass refuses.
/// `gm_post_run`'s body.
pub(crate) fn post_pass(
    topology: &Topology,
    geometry: &Geometry,
    index: u32,
) -> Result<Snapshot, Code> {
    let ran = crate::post::snapshot(index, topology, geometry).ok_or(Code::IndexOutOfRange)?;
    ran.map_err(|_| Code::PostFailed)
}

/// The registry index of the POST capability named `id`.
fn post_index(id: &str) -> Result<u32, Code> {
    (0..crate::post::count())
        .find(|&index| crate::post::id_at(index) == Some(id))
        .ok_or(Code::IndexOutOfRange)
}

#[cfg(test)]
mod tests;
