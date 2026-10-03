//! Graph lifecycle: `gm_build` through a run's geometry tags, plus the two exports that
//! do not touch a handle at all (`gm_last_error`, `gm_seed_ingest`). Reading a finished
//! run's column/snapshot data back out is `super::columns` instead (the 300-line split).

use super::state::{HANDLES, publish};
use crate::alloc::is_live;
use crate::contract;
use crate::errors::{self, Code};
use crate::handle::Handle;
use crate::ingest;
use crate::seed_ingest;
use crate::views;
use graph_contract::binary::Snapshot;
use graph_core::registry::LAYOUTS;
use graph_core::{Geometry, StageError, Topology};

/// Registry-driven layout count (C1). p3's four new rows change this with no ABI change.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_layout_count() -> u32 {
    errors::clear();
    u32::try_from(LAYOUTS.len()).unwrap_or(0)
}

/// The capability id at registry index `i`, framed UTF-8; `0` if `i` is out of range.
/// `gm_run`'s `layout_id` *is* this index — the SDK maps a string id to it by scanning
/// `0..gm_layout_count()` at init, never by a hard-coded constant (C1).
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_layout_id(i: u32) -> u32 {
    match LAYOUTS.get(i as usize) {
        Some(layout) => {
            errors::clear();
            publish(layout.id.as_bytes().to_vec())
        }
        None => {
            errors::set(Code::IndexOutOfRange);
            0
        }
    }
}

/// Builds a graph from the provisional-ingest buffer at `(ingest_ptr, ingest_len)`,
/// which must be a live `gm_alloc` allocation (C5) — this copies out of it and never
/// frees it; the caller frees it once this returns (C7). `0` on any refusal.
// SAFETY: as `gm_layout_count`. The exact byte range read is confirmed live by
// `is_live` immediately before the one slice formed from it, and that slice does not
// outlive this call.
#[unsafe(no_mangle)]
pub extern "C" fn gm_build(ingest_ptr: u32, ingest_len: u32) -> u32 {
    if !is_live(ingest_ptr, ingest_len) {
        errors::set(Code::BuildSourceInvalid);
        return 0;
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns; the buffer outlives this whole call (freed only by the
    // caller's own later `gm_free`), so borrowing it for the duration of `ingest::read`
    // is sound, and nothing here retains the slice past this function.
    let bytes = unsafe { std::slice::from_raw_parts(ingest_ptr as *const u8, ingest_len as usize) };
    #[cfg(any(test, feature = "probe"))]
    crate::ingest::phases::mark(crate::ingest::phases::COPY, None);
    // The records are dropped as soon as the topology holds them, not at the end of the call.
    let indexed =
        ingest::read_records(bytes).and_then(|(nodes, edges)| ingest::index(&nodes, &edges));
    #[cfg(any(test, feature = "probe"))]
    crate::ingest::phases::mark(crate::ingest::phases::RETURNED, None);
    let topology = match indexed {
        Ok(topology) => topology,
        // F-16: the refusal names its own code, so an oversized document is not published
        // as a malformed one.
        Err(refusal) => {
            errors::set(refusal.code());
            return 0;
        }
    };
    insert(topology)
}

/// Builds a graph from the **ingest contract** buffer at `(contract_ptr, contract_len)`,
/// which must be a live `gm_alloc` allocation (C5) with the same ownership rule as
/// [`gm_build`]: copied out of, never freed, the caller's to free either way. `0` on any
/// refusal.
///
/// **Additive.** [`gm_build`] and its provisional node/edge JSON are unchanged — that is
/// the format the host studio and the hash gate's C20 stage already speak, and both still
/// work. This export is the *other* way in, for the phase-10 contract
/// (`docs/contract/ingest-schema.json`): the document declares roles, and the graph comes
/// from `graph_core::ingest`'s single derivation, which is the whole point of the
/// contract — the motor holds one derivation instead of one per source.
///
/// The two formats are deliberately not interchangeable and each reader refuses the
/// other's document, so this cannot become `gm_build` by accident: routing it to the
/// provisional parser would derive nothing at all, and routing `gm_build` here would
/// refuse every document the studio sends. Both directions are pinned by tests in
/// `crate::contract`.
///
/// Refusals are `Code::ContractInvalid` for a document that is not a valid contract or
/// describes a graph that cannot be derived, `Code::BuildSourceInvalid` for a
/// `(ptr, len)` that is not a live allocation, and `Code::HandlesExhausted` when every
/// handle id has been issued. Not `IngestInvalid`: see [`Code::ContractInvalid`].
// SAFETY: as `gm_layout_count`. The byte range read is confirmed live by `is_live`
// immediately before the one slice formed from it, and that slice does not outlive this
// call — `contract::derive` borrows it and returns only owned records and a topology.
#[unsafe(no_mangle)]
pub extern "C" fn gm_build_contract(contract_ptr: u32, contract_len: u32) -> u32 {
    if !is_live(contract_ptr, contract_len) {
        errors::set(Code::BuildSourceInvalid);
        return 0;
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns; the buffer outlives this whole call (freed only by the caller's
    // own later `gm_free`), so borrowing it for the duration of `contract::derive` is
    // sound, and nothing here retains the slice past this function.
    let bytes =
        unsafe { std::slice::from_raw_parts(contract_ptr as *const u8, contract_len as usize) };
    let Ok((_, topology)) = contract::derive(bytes) else {
        errors::set(Code::ContractInvalid);
        return 0;
    };
    insert(topology)
}

/// Runs registry layout `layout_id` (an index, C1) over `handle`'s topology at its
/// default parameters — the registry's `run: fn(&Topology)` takes none (C2), so
/// `params_len` must be `0`; any other value is refused, never silently ignored. `1` on
/// success, `0` on any refusal. Every refusal clears the handle's previous geometry
/// first, so a failed run never leaves a stale snapshot to be served (C4).
// SAFETY: as `gm_layout_count`. `params_ptr` is never read: an empty params buffer
// carries no bytes to read, and a non-empty one is refused before any read would occur.
#[unsafe(no_mangle)]
pub extern "C" fn gm_run(handle: u32, layout_id: u32, params_ptr: u32, params_len: u32) -> u32 {
    let _ = params_ptr;
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        let Some(entry) = handles.get_mut(handle) else {
            errors::set(Code::InvalidHandle);
            return 0;
        };
        entry.snapshot = None;
        entry.geometry = None;
        if params_len != 0 {
            errors::set(Code::ParamsMustBeEmpty);
            return 0;
        }
        let Some(layout) = LAYOUTS.get(layout_id as usize) else {
            errors::set(Code::UnknownLayoutId);
            return 0;
        };
        let ran = (layout.run)(&entry.topology);
        store(entry, ran)
    })
}

/// A new handle over `topology`, or `0` with [`Code::HandlesExhausted`].
pub(super) fn insert(topology: Topology) -> u32 {
    let handle = Handle {
        topology,
        snapshot: None,
        geometry: None,
    };
    match HANDLES.with(|handles| handles.borrow_mut().insert(handle)) {
        Some(id) => {
            errors::clear();
            id
        }
        None => {
            errors::set(Code::HandlesExhausted);
            0
        }
    }
}

/// Keeps a layout's result on `entry` with its snapshot: `1`, or `0` with
/// [`Code::LayoutFailed`] and nothing kept.
pub(super) fn store(entry: &mut Handle, ran: Result<Geometry, StageError>) -> u32 {
    let ran = ran.map_err(|_| Code::LayoutFailed).and_then(|geometry| {
        graph_core::layout::snapshot(&entry.topology, geometry.clone())
            .map(|snapshot| (geometry, snapshot))
            .map_err(|_| Code::LayoutFailed)
    });
    match ran {
        Ok((geometry, snapshot)) => {
            entry.geometry = Some(geometry);
            entry.snapshot = Some(snapshot);
            errors::clear();
            1
        }
        Err(code) => {
            errors::set(code);
            0
        }
    }
}

/// Nodes in `handle`'s topology — available right after `gm_build`, before any run.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_node_count(handle: u32) -> u32 {
    HANDLES.with(|handles| match handles.borrow().get(handle) {
        Some(entry) => {
            errors::clear();
            entry.topology.node_count()
        }
        None => {
            errors::set(Code::InvalidHandle);
            0
        }
    })
}

/// The node geometry tag (`0` Point, `1` Circle, `2` Box) of `handle`'s last successful
/// run, or `u32::MAX` — never a real tag — before any run has succeeded.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_geometry_kind(handle: u32) -> u32 {
    with_snapshot(handle, |snapshot| u32::from(views::node_kind_tag(snapshot)))
}

/// The edge geometry tag (`0` Line, `1` Polyline, `2` Curve). An export beyond the
/// phase's minimum surface: `gm_geometry_kind` alone only names nodes, and edge kind
/// must be readable too (C3). `u32::MAX` before any run has succeeded.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_edge_geometry_kind(handle: u32) -> u32 {
    with_snapshot(handle, |snapshot| u32::from(views::edge_kind_tag(snapshot)))
}

fn with_snapshot(handle: u32, read: impl FnOnce(&Snapshot) -> u32) -> u32 {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let Some(entry) = handles.get(handle) else {
            errors::set(Code::InvalidHandle);
            return u32::MAX;
        };
        let Some(snapshot) = &entry.snapshot else {
            errors::set(Code::NoGeometryYet);
            return u32::MAX;
        };
        errors::clear();
        read(snapshot)
    })
}

/// Why the most recent call returned its failure sentinel; `0` ([`Code::None`]) after a
/// call that succeeded (C4). Read-only: calling it does not itself change the code, so
/// it can be polled after any other export without disturbing what it would report.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_last_error() -> u32 {
    errors::get()
}

/// Gate-only: the linear-memory address of [`crate::ingest::phases`]' mark table, so a
/// host can read the per-phase marks straight out of `memory.buffer` — including after a
/// trap, which is the only way to see the phases of a document that does not finish. Not
/// part of the published SDK surface; compiled out of the default artifact.
// SAFETY: `gm_probe_base` is the only symbol with this name, and it reads no memory.
#[cfg(any(test, feature = "probe"))]
#[unsafe(no_mangle)]
pub extern "C" fn gm_probe_base() -> u32 {
    u32::try_from(crate::ingest::phases::base()).unwrap_or(0)
}

/// Gate-only: as `gm_build` records them, a fresh run's marks.
// SAFETY: as `gm_probe_base`.
#[cfg(any(test, feature = "probe"))]
#[unsafe(no_mangle)]
pub extern "C" fn gm_probe_reset() -> u32 {
    crate::ingest::phases::reset();
    1
}

/// Gate-only: the hash gate's model at `seed`, as the provisional ingest JSON `gm_build`
/// reads (C20). Not part of the published SDK surface; `harness/sdk-smoke.mjs` never
/// calls it, only `harness/wasm-run.mjs`'s hash mode does.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_seed_ingest(seed: u32) -> u32 {
    match seed_ingest::for_seed(seed) {
        Some(text) => publish(text.into_bytes()),
        None => errors::reply(Err(Code::IngestInvalid)),
    }
}
