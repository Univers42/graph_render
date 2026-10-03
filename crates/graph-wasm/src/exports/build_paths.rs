//! The three ways a graph arrives: provisional node/edge JSON ([`gm_build`]), the
//! phase-10 ingest contract ([`gm_build_contract`]) and the columnar document
//! ([`gm_build_columns`]). Split from [`super::build`] for the house line limit; all three
//! share one handle lifecycle, which is the [`store`] at the bottom rather than a copy of it.

use super::state::HANDLES;
use crate::alloc::is_live;
use crate::contract;
use crate::errors::{self, Code};
use crate::handle::Handle;
use crate::ingest::{self, columns};
use graph_core::Topology;

/// Builds a graph from the provisional-ingest buffer at `(ingest_ptr, ingest_len)`,
/// which must be a live `gm_alloc` allocation (C5) — this copies out of it and never
/// frees it; the caller frees it once this returns (C7). `0` on any refusal.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_build`. The exact byte range read is confirmed live by
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
    // The records are dropped as soon as the topology holds them, not at the end of the call.
    let indexed =
        ingest::read_records(bytes).and_then(|(nodes, edges)| ingest::index(&nodes, &edges));
    let topology = match indexed {
        Ok(topology) => topology,
        // F-16: the refusal names its own code, so an oversized document is not published
        // as a malformed one.
        Err(refusal) => {
            errors::set(refusal.code());
            return 0;
        }
    };
    store(topology)
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
// SAFETY: as `gm_build`. The byte range read is confirmed live by `is_live`
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
    store(topology)
}

/// Builds a graph from the **columnar ingest** buffer at `(columns_ptr, columns_len)`,
/// which must be a live `gm_alloc` allocation (C5) with the same ownership rule as
/// [`gm_build`]: copied out of, never freed, the caller's to free either way. `0` on any
/// refusal.
///
/// **Additive**, like [`gm_build_contract`] before it: the JSON reader and its refusal code
/// are untouched, and each reader refuses the other formats. The document is the one
/// `docs/contract/ingest-columns.md` specifies — one UTF-8 string table, `u32` and `f64`
/// columns, edge endpoints as node row numbers — so the graph is built without a JSON parse,
/// without a `String` per record field, and without one arena probe per edge endpoint.
///
/// The one behavioural difference from `gm_build`, and the reason the format exists: a
/// repeated node or edge id is **refused**, not dropped first-wins. A row-addressed endpoint
/// cannot survive a dropped row, because dropping renumbers every row after it and silently
/// repoints every edge that follows.
///
/// Refusals are `Code::ColumnsInvalid` for anything the contract or `index_columns` rejects,
/// `Code::IngestTooLarge` for a buffer over `ingest::MAX_INGEST_BYTES` (checked before any
/// decode), `Code::BuildSourceInvalid` for a `(ptr, len)` that is not a live allocation, and
/// `Code::HandlesExhausted` when every handle id has been issued.
// SAFETY: as `gm_build`. The byte range read is confirmed live by `is_live`
// immediately before the one slice formed from it, and that slice does not outlive this
// call — `columns::index` borrows it and returns an owned topology.
#[unsafe(no_mangle)]
pub extern "C" fn gm_build_columns(columns_ptr: u32, columns_len: u32) -> u32 {
    if !is_live(columns_ptr, columns_len) {
        errors::set(Code::BuildSourceInvalid);
        return 0;
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns; the buffer outlives this whole call (freed only by the caller's
    // own later `gm_free`), so borrowing it for the duration of `columns::index` is sound,
    // and nothing here retains the slice past this function.
    let bytes = unsafe { std::slice::from_raw_parts(columns_ptr as *const u8, columns_len as usize) };
    let topology = match columns::index(bytes) {
        Ok(topology) => topology,
        Err(refusal) => {
            errors::set(refusal.code());
            return 0;
        }
    };
    store(topology)
}
/// The handle lifecycle all three builds share: a live topology becomes handle `1` or more,
/// `0` with [`Code::HandlesExhausted`] when every id has been issued. Ids are monotonic and
/// never reused (C6), so a caller that sees `0` knows no handle was consumed.
fn store(topology: Topology) -> u32 {
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
