//! Browser threads model (a) (`prompts/perf-plan.md` P3): helper instances over this module's
//! shared memory join [`POOL`]; `gm_run_threaded` runs a force layout and
//! `gm_force_session_tick_threaded` ticks a live session, with their gathers divided across
//! them. The host's side of the protocol is `harness/wasm-threads.mjs`:
//! the coordinator takes a stack and a TLS block per helper from `gm_thread_block`, and each
//! helper instance sets its `__stack_pointer`, calls `__wasm_init_tls`, then
//! `gm_thread_serve`. The coordinator itself must be allowed to block (a Worker, or Node's
//! main thread), since a pass waits on its helpers.

use super::build::{insert, store};
use super::state::HANDLES;
use crate::errors::{self, Code};
use crate::pool::{Pool, PoolRunner};
use graph_core::layout::force::{BarnesHut, ForceParams, ParticleMesh};
use graph_core::registry::LAYOUTS;
use graph_core::{REFERENCE_DEGREE, Stage, index_model, seeded_model};
use std::alloc::{Layout, alloc};

/// One pool per module: every instance over the shared memory sees this same static.
static POOL: Pool = Pool::new();

/// `len` bytes aligned to 16, for a helper's stack or TLS block, or `0` when `len` is 0 or
/// memory is out. Never freed: a helper lives as long as the module.
// SAFETY: as `gm_layout_count`; no other symbol in this crate has this name.
#[unsafe(no_mangle)]
pub extern "C" fn gm_thread_block(len: u32) -> u32 {
    let Ok(layout) = Layout::from_size_align(len as usize, 16) else {
        return 0;
    };
    if len == 0 {
        return 0;
    }
    // SAFETY: `layout` has a non-zero size, checked above.
    let ptr = unsafe { alloc(layout) };
    u32::try_from(ptr as usize).unwrap_or(0)
}

/// Joins the pool as the next helper and runs the parts addressed to it; returns after
/// `gm_thread_close`.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_thread_serve() {
    POOL.serve();
}

/// Helpers that have joined; a host waits for its count before the first run.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_thread_count() -> u32 {
    POOL.helpers()
}

/// Releases every helper from `gm_thread_serve`, so their workers can exit. Final: a run
/// after it is serial.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_thread_close() {
    POOL.close();
}

/// The gate model at `seed` with `n` nodes, straight into a handle: no ingest text, which at
/// a million nodes would cost more than the layout being measured. `0` on a refusal.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_seed_handle(seed: u32, n: u32) -> u32 {
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    match index_model(&nodes, &edges) {
        Ok(topology) => insert(topology),
        Err(_) => {
            errors::set(Code::IngestInvalid);
            0
        }
    }
}

/// `gm_run` with the default parameters, over the pool: `workers` parts, capped at the
/// helpers present plus this thread. Barnes-Hut and particle-mesh only, the two layouts with
/// a `run_with`. Bit 0 of `flags` is the negative control: the last part writes nothing.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_run_threaded(handle: u32, layout_id: u32, workers: u32, flags: u32) -> u32 {
    let runner = PoolRunner {
        pool: &POOL,
        skip_last: flags & 1 != 0,
    };
    let params = ForceParams::default();
    let id = LAYOUTS.get(layout_id as usize).map(|layout| layout.id);
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        let Some(entry) = handles.get_mut(handle) else {
            errors::set(Code::InvalidHandle);
            return 0;
        };
        entry.snapshot = None;
        entry.geometry = None;
        let ran = if id == Some(BarnesHut::ID) {
            BarnesHut::run_with(&entry.topology, &params, &runner, workers)
        } else if id == Some(ParticleMesh::ID) {
            ParticleMesh::run_with(&entry.topology, &params, &runner, workers)
        } else {
            errors::set(Code::UnknownLayoutId);
            return 0;
        };
        store(entry, ran)
    })
}

/// `gm_force_session_tick` over the pool: the live session's gathers in `workers` parts, the
/// same status word and the same columns as the serial tick. Bit 0 of `flags` is the negative
/// control, as for `gm_run_threaded`. `0` on a refusal, which `gm_last_error` names.
// SAFETY: as `gm_thread_block`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_tick_threaded(
    session: u32,
    ticks: u32,
    workers: u32,
    flags: u32,
) -> u32 {
    let runner = PoolRunner {
        pool: &POOL,
        skip_last: flags & 1 != 0,
    };
    match crate::session::tick_with(session, ticks, &runner, workers) {
        Ok(status) => {
            errors::clear();
            status.as_u32()
        }
        Err(code) => {
            errors::set(code);
            0
        }
    }
}
