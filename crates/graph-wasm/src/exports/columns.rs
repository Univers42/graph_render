//! Reading a finished run's data back out: columns, the two snapshot faces, and
//! `gm_release`. `super::build` is graph lifecycle up to a successful `gm_run` (the
//! 300-line split).

use super::state::{HANDLES, publish};
use crate::errors::{self, Code};
use crate::views;
use graph_contract::binary::Snapshot;

/// Offset of column `column_id`'s data for `handle`'s last run; `0` if the handle is
/// invalid, there is no geometry yet, or the id is reserved/inapplicable (C3) — an
/// ambiguous `0`, resolved by `gm_last_error` (C4).
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_column_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_column_ptr(handle: u32, column_id: u32) -> u32 {
    resolve_column(handle, column_id, false)
}

/// Element count of column `column_id` for `handle`'s last run; never assumes the node
/// or edge count — a notes column (Phase 3, reserved here) has its own length `k` (C3).
// SAFETY: as `gm_column_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_column_len(handle: u32, column_id: u32) -> u32 {
    resolve_column(handle, column_id, true)
}

/// How many dimensions `handle`'s last run carries: `0` 2D, `1` 3D. The wasm layer
/// transports 3D rather than refusing it, so this is a reading, not a refusal: a consumer
/// that draws in 2D checks this and declines; one that can draw in 3D reads a `z` column
/// through `gm_column_ptr`/`gm_column_len` with `ColumnId::NodeZ` (12).
// SAFETY: as `gm_column_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_dim(handle: u32) -> u32 {
    HANDLES.with(|handles| {
        let dim = handles.borrow().snapshot(handle).map(views::dim);
        errors::reply(dim.map(u32::from))
    })
}

fn resolve_column(handle: u32, column_id: u32, want_len: bool) -> u32 {
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let snapshot = handles.snapshot(handle);
        errors::reply(snapshot.and_then(|s| views::column_wire(s, column_id, want_len)))
    })
}

/// The canonical JSON face of `handle`'s last run, framed UTF-8. Re-validates every
/// coordinate as finite first (D9, C8): the SDK's column views are writable aliases
/// directly into this snapshot's buffers, and neither face re-checks on its own.
// SAFETY: as `gm_column_ptr`.
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
// SAFETY: as `gm_column_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_snapshot_bytes(handle: u32) -> u32 {
    with_valid_snapshot(handle, Snapshot::to_bytes)
}

fn with_valid_snapshot(handle: u32, encode: impl FnOnce(&Snapshot) -> Vec<u8>) -> u32 {
    HANDLES.with(|handles| match handles.borrow().snapshot(handle) {
        Ok(snapshot) if !views::has_non_finite(snapshot) => {
            errors::clear();
            publish(encode(snapshot))
        }
        Ok(_) => errors::reply(Err(Code::TamperedGeometry)),
        Err(code) => errors::reply(Err(code)),
    })
}

/// Releases `handle`. The id is never reissued (C6); using it again after this always
/// reads `InvalidHandle`, never the next graph built.
// SAFETY: as `gm_column_ptr`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_release(handle: u32) {
    match HANDLES.with(|handles| handles.borrow_mut().remove(handle)) {
        Some(_) => errors::clear(),
        None => errors::set(Code::InvalidHandle),
    }
}
