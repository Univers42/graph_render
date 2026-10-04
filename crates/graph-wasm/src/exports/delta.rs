//! Live growth on the wire (`docs/contract/delta.md` "The wasm ABI: two exports"):
//! [`gm_graph_extend`] appends a batch to a built graph, and [`gm_force_session_grow`] takes a
//! session over that graph onto what was appended. Both are frames over the
//! target-independent [`crate::service::extend`] and [`crate::session::grow`] (C21), so a
//! native `cargo test` reaches every refusal but the buffer read itself.

use super::session::{answered, refuse};
use super::state::HANDLES;
use crate::alloc::is_live;
use crate::errors::Code;
use crate::service;
use crate::session;

#[cfg(test)]
mod tests;

/// Appends the provisional node/edge batch at `(ptr, len)` — the document `gm_build` reads,
/// holding only the new nodes and edges — to `graph`'s topology. The buffer must be a live
/// `gm_alloc` allocation (C5); it is copied out of and never freed, so the caller frees it
/// either way (C7). `1` appended, `0` refused.
///
/// Refusals, in the order they are checked, each leaving the graph unchanged:
/// `InvalidHandle` for a graph that is not live, `BuildSourceInvalid` for a buffer that is
/// not a live allocation (as `gm_build`), `IngestTooLarge` and `IngestInvalid` for a batch
/// `gm_build` would refuse as a document, and `IngestInvalid` for one the graph refuses — an
/// id it already holds, an endpoint that names no node.
///
/// Success clears the handle's last run, as a failed `gm_run` does (C4): that geometry was
/// laid over the graph before the batch, and serving it would answer for a graph that no
/// longer exists. A session over the graph sees the batch only after `gm_force_session_grow`.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_graph_extend`. The exact byte range read is confirmed live by
// `is_live` immediately before the one slice formed from it, and that slice does not
// outlive this call.
#[unsafe(no_mangle)]
pub extern "C" fn gm_graph_extend(graph: u32, ptr: u32, len: u32) -> u32 {
    if !HANDLES.with(|handles| handles.borrow().get(graph).is_some()) {
        return refuse(Code::InvalidHandle, 0);
    }
    if !is_live(ptr, len) {
        return refuse(Code::BuildSourceInvalid, 0);
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns; the buffer outlives this whole call (freed only by the caller's own
    // later `gm_free`), so borrowing it for the duration of `extend` is sound, and nothing
    // here retains the slice past this function.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    answered(extend(graph, bytes), 1)
}

/// Grows `session` onto the rows and edges `gm_graph_extend` appended to `graph` since the
/// session was created or last grown: O(batch), and the same bits a fresh session carried
/// across from the old graph would hold (`ForceSession::grow`). `1` grown, `0` refused.
///
/// Refusals, each leaving the session unchanged: `InvalidSession` for a session that is not
/// live, `InvalidHandle` for a graph that is not, and `SessionRefused` for a graph other than
/// the one the session was created over, or a growth past the session's adjacency limit. A
/// grow with nothing appended succeeds and changes nothing. The session's column addresses
/// are stale after a success (C7).
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_force_session_grow`. It reads no pointer.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_grow(session: u32, graph: u32) -> u32 {
    let grown = HANDLES.with(|handles| {
        let handles = handles.borrow();
        session::grow(session, graph, handles.get(graph).map(|h| &h.topology))
    });
    answered(grown, 1)
}

/// `gm_graph_extend` past the buffer read: the batch appended to `graph`'s topology, and its
/// last run cleared, or the refusal with the handle untouched.
fn extend(graph: u32, bytes: &[u8]) -> Result<(), Code> {
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        let entry = handles.get_mut(graph).ok_or(Code::InvalidHandle)?;
        service::extend(&mut entry.topology, bytes)?;
        entry.snapshot = None;
        entry.geometry = None;
        Ok(())
    })
}
