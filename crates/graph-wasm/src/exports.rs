//! The ABI's `extern "C"` surface beyond the retained hash-gate shims
//! (`docs/contract/wasm-abi.md` is authoritative; `crate::gate_exports` is the shim
//! module C20 keeps green). Wasm32-only: this is the pointer layer over the
//! target-independent `alloc`, `handle`, `ingest`, `views`, `seed_ingest` and `errors`
//! modules (C21) — every one of those is unit-tested natively; only the functions below
//! need the real target to exist at all.

#![cfg(target_arch = "wasm32")]

use crate::alloc::is_live;
use crate::errors::{self, Code};
use crate::handle::{Handle, Handles};
use crate::ingest;
use crate::seed_ingest;
use crate::views::{self, Column};
use graph_contract::binary::Snapshot;
use graph_core::index_model;
use graph_core::registry::LAYOUTS;
use std::cell::RefCell;

thread_local! {
    static HANDLES: RefCell<Handles> = RefCell::new(Handles::new());
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Frames `bytes` as `[len: u32 LE][len bytes]` in the shared out-buffer and returns its
/// address. The motor owns this buffer; it is valid until the next motor call (C7).
fn publish(bytes: Vec<u8>) -> u32 {
    let Ok(len) = u32::try_from(bytes.len()) else {
        errors::set(Code::AllocFailed);
        return 0;
    };
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        out.clear();
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
        u32::try_from(out.as_ptr() as usize).unwrap_or(0)
    })
}

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
    let Ok((nodes, edges)) = ingest::read(bytes) else {
        errors::set(Code::IngestInvalid);
        return 0;
    };
    let Ok(topology) = index_model(&nodes, &edges) else {
        errors::set(Code::IngestInvalid);
        return 0;
    };
    let handle = Handle {
        topology,
        snapshot: None,
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
        if params_len != 0 {
            errors::set(Code::ParamsMustBeEmpty);
            return 0;
        }
        let Some(layout) = LAYOUTS.get(layout_id as usize) else {
            errors::set(Code::UnknownLayoutId);
            return 0;
        };
        let ran = (layout.run)(&entry.topology)
            .map_err(|_| Code::LayoutFailed)
            .and_then(|geometry| {
                graph_core::layout::snapshot(&entry.topology, geometry)
                    .map_err(|_| Code::LayoutFailed)
            });
        match ran {
            Ok(snapshot) => {
                entry.snapshot = Some(snapshot);
                errors::clear();
                1
            }
            Err(code) => {
                errors::set(code);
                0
            }
        }
    })
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

/// Offset of column `column_id`'s data for `handle`'s last run; `0` if the handle is
/// invalid, there is no geometry yet, or the id is reserved/inapplicable (C3) — an
/// ambiguous `0`, resolved by `gm_last_error` (C4).
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_column_ptr(handle: u32, column_id: u32) -> u32 {
    resolve_column(handle, column_id, false)
}

/// Element count of column `column_id` for `handle`'s last run; never assumes the node
/// or edge count — a notes column (Phase 3, reserved here) has its own length `k` (C3).
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_column_len(handle: u32, column_id: u32) -> u32 {
    resolve_column(handle, column_id, true)
}

fn resolve_column(handle: u32, column_id: u32, want_len: bool) -> u32 {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let Some(entry) = handles.get(handle) else {
            errors::set(Code::InvalidHandle);
            return 0;
        };
        let Some(snapshot) = &entry.snapshot else {
            errors::set(Code::NoGeometryYet);
            return 0;
        };
        errors::clear();
        let (ptr, len) = match views::column(snapshot, column_id) {
            Column::Absent => return 0,
            Column::F32(v) => (v.as_ptr() as usize, v.len()),
            Column::U32(v) => (v.as_ptr() as usize, v.len()),
        };
        u32::try_from(if want_len { len } else { ptr }).unwrap_or(0)
    })
}

/// The canonical JSON face of `handle`'s last run, framed UTF-8. Re-validates every
/// coordinate as finite first (D9, C8): the SDK's column views are writable aliases
/// directly into this snapshot's buffers, and neither face re-checks on its own.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_snapshot_json(handle: u32) -> u32 {
    with_valid_snapshot(handle, |snapshot| {
        graph_contract::canonical_json::to_json(snapshot).into_bytes()
    })
}

/// The binary face of `handle`'s last run, framed. Same D9 re-validation as
/// `gm_snapshot_json`. An extra export beyond the phase's minimum surface, needed so
/// `harness/wasm-run.mjs`'s hash mode can compare the real-ABI path against the retained
/// `gm_layout_grid` shim's bytes (C20): both are the binary face, so their hashes are
/// directly comparable.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_snapshot_bytes(handle: u32) -> u32 {
    with_valid_snapshot(handle, Snapshot::to_bytes)
}

fn with_valid_snapshot(handle: u32, encode: impl FnOnce(&Snapshot) -> Vec<u8>) -> u32 {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let Some(entry) = handles.get(handle) else {
            errors::set(Code::InvalidHandle);
            return 0;
        };
        let Some(snapshot) = &entry.snapshot else {
            errors::set(Code::NoGeometryYet);
            return 0;
        };
        if views::has_non_finite(snapshot) {
            errors::set(Code::TamperedGeometry);
            return 0;
        }
        errors::clear();
        publish(encode(snapshot))
    })
}

/// Releases `handle`. The id is never reissued (C6); using it again after this always
/// reads `InvalidHandle`, never the next graph built.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_release(handle: u32) {
    match HANDLES.with(|handles| handles.borrow_mut().remove(handle)) {
        Some(_) => errors::clear(),
        None => errors::set(Code::InvalidHandle),
    }
}

/// Why the most recent call returned its failure sentinel; `0` ([`Code::None`]) after a
/// call that succeeded (C4). Read-only: calling it does not itself change the code, so
/// it can be polled after any other export without disturbing what it would report.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_last_error() -> u32 {
    errors::get()
}

/// Gate-only: the hash gate's model at `seed`, as the provisional ingest JSON `gm_build`
/// reads (C20). Not part of the published SDK surface; `harness/sdk-smoke.mjs` never
/// calls it, only `harness/wasm-run.mjs`'s hash mode does.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_seed_ingest(seed: u32) -> u32 {
    errors::clear();
    publish(seed_ingest::for_seed(seed).into_bytes())
}
