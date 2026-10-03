//! Graph lifecycle past the build: a run, the counts and geometry tags it produces, and the
//! two exports that touch no handle at all (`gm_last_error`, `gm_seed_ingest`). The three
//! build paths are [`super::build_paths`], and reading a finished run's data back out is
//! [`super::columns`] — both splits are the house's 300-line limit, not an ABI grouping.

use super::state::{HANDLES, publish};
use crate::errors::{self, Code};
use crate::handle::Handle;
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

/// A new handle over `topology`, or `0` with [`Code::HandlesExhausted`]. Ids are monotonic and
/// never reused (C6), so a caller that sees `0` knows no handle was consumed.
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
