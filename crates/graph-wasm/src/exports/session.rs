//! The `extern "C"` layer over [`crate::session`]: the `gm_force_session_*` family, the ABI
//! a live force host drives. `docs/decisions/force-wasm-abi.md` is the table of record.
//!
//! Every function below is a frame: read the arguments, call into the target-independent
//! module, record the code, return the wire's word. The physics, the refusals and the id table
//! are all in [`crate::session`], so a native `cargo test` covers every branch of them and the
//! only thing here that genuinely needs wasm32 is the address arithmetic at the bottom of this
//! file (C21).
//!
//! **`f64` crosses this ABI in both directions**, unlike the rest of it: `alpha` is the value
//! an interactive loop is driving on, and the pins' arguments are coordinates a view already
//! holds as doubles. It is a fixed-width IEEE-754 value, not a pointer-width one, so it
//! carries no `usize` across the wire (D6) and is bit-identical on both targets — which is
//! what makes the native-vs-wasm32 check over these columns a check of the tick rather than of
//! the transport.

use super::state::{HANDLES, publish};
use crate::alloc::is_live;
use crate::errors::{self, Code};
use crate::session;
use crate::session::Engine;
use crate::session::params;
use graph_core::layout::force::LiveParams;

/// A session over the graph `graph` names, at the parameter buffer at
/// `(params_ptr, params_len)`.
///
/// `params_len` is `0` for the compiled-in defaults, or [`session::PARAMS_LEN`] for thirteen
/// little-endian `f64`s in a live `gm_alloc` allocation (C5) — copied out of and never freed,
/// so the caller frees it either way (C7). Any other length is refused by name
/// ([`Code::SessionParamsInvalid`]) rather than read as the defaults.
///
/// Refusals: `InvalidHandle` for a graph that is not live, `SessionParamsInvalid` for a
/// `(ptr, len)` that is not a live allocation of that length, `SessionRefused` for parameters
/// out of range (never clamped), and `HandlesExhausted` when every session id has been issued.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this crate
// is named `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_create(graph: u32, params_ptr: u32, params_len: u32) -> u32 {
    create(graph, (params_ptr, params_len), Engine::BarnesHut)
}

/// [`gm_force_session_create`] for a session that ticks on the particle mesh: the same
/// arguments, refusals and session id space; every other `gm_force_session_*` call takes it.
/// Its position columns move on every tick, so a host re-reads `column_ptr` after each one.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_create_mesh(
    graph: u32,
    params_ptr: u32,
    params_len: u32,
) -> u32 {
    create(graph, (params_ptr, params_len), Engine::ParticleMesh)
}

fn create(graph: u32, (params_ptr, params_len): (u32, u32), engine: Engine) -> u32 {
    let params = match read_params(params_ptr, params_len) {
        Ok(params) => params,
        Err(code) => return refuse(code, 0),
    };
    // The topology is read inside this closure and never borrowed out of it: `create` copies
    // everything it needs out of the graph and keeps no reference, so the host is free to
    // release the handle the moment this returns.
    HANDLES.with(|handles| {
        let handles = handles.borrow();
        let Some(handle) = handles.get(graph) else {
            return refuse(Code::InvalidHandle, 0);
        };
        match session::create(graph, &handle.topology, params, engine) {
            Ok(id) => {
                errors::clear();
                id
            }
            Err(code) => refuse(code, 0),
        }
    })
}

/// Replaces the session's parameters, from the same buffer shape
/// [`gm_force_session_create`] takes. `1` on success, `0` on any refusal — which leaves the
/// session exactly as it was.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_set_params(
    session: u32,
    params_ptr: u32,
    params_len: u32,
) -> u32 {
    let params = match read_params(params_ptr, params_len) {
        Ok(params) => params,
        Err(code) => return refuse(code, 0),
    };
    answered(session::set_params(session, params), 1)
}

/// The parameters in force, framed as thirteen little-endian `f64`s — the same buffer shape
/// the two setters read, so a host reads the defaults once and sends a whole set back rather
/// than a copy of them in its own source.
///
/// It exists because the alternative is thirteen defaults written out in TypeScript, where a
/// change to `LiveParams::default` would leave a host sending the old ones and nothing would
/// say so.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_params(session: u32) -> u32 {
    match session::params_of(session) {
        Ok(params) => {
            errors::clear();
            publish(params::encode(&params))
        }
        Err(code) => refuse(code, 0),
    }
}

/// Runs `ticks` ticks: `0` refused, `1` ran and is still cooling, `2` ran and has settled.
/// Three words rather than a bool, because "settled" and "still moving" are the two answers
/// an interactive loop drives on and `0` is already the refusal.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_tick(session: u32, ticks: u32) -> u32 {
    match session::tick(session, ticks) {
        Ok(status) => {
            errors::clear();
            status.as_u32()
        }
        Err(code) => refuse(code, 0),
    }
}

/// The cooling schedule's current value. `0.0` is both the refusal value and a value a real
/// run can hold, so `gm_last_error` resolves it — the same ambiguity `0` already has
/// everywhere else in this ABI, resolved the same way (C4).
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_alpha(session: u32) -> f64 {
    match session::alpha(session) {
        Ok(alpha) => {
            errors::clear();
            alpha
        }
        Err(code) => {
            errors::set(code);
            0.0
        }
    }
}

/// Sets `alpha` outright — d3's `alpha(_)`, the verb behind "the user moved something, run it
/// again". `0..=1` exactly, never clamped; anything else is [`Code::SessionRefused`]. `1` on
/// success, `0` on any refusal.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_reheat(session: u32, alpha: f64) -> u32 {
    answered(session::reheat(session, alpha), 1)
}

/// Holds `row` at `(x, y)` from the next tick on — **and this is the drag verb too**: a drag
/// is a pin at a new point, and graph-core has one method for both (d3's `node.fx`/`node.fy`).
///
/// `row` is the node's row in the two position columns, the dense order `gm_column_ptr`'s
/// `NodeX` addresses; no node-id string crosses this ABI. `1` on success, `0` on a row past
/// the last node or a coordinate that is not finite (D9).
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_pin(session: u32, row: u32, x: f64, y: f64) -> u32 {
    answered(session::pin(session, row, x, y), 1)
}

/// Releases one row, which then integrates again from rest. `1` on success, `0` on a refusal —
/// and an unpin of a row that was never pinned is a no-op a caller may legitimately mean, so
/// it succeeds. Only a row past the last node is refused.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_unpin(session: u32, row: u32) -> u32 {
    answered(session::unpin(session, row), 1)
}

/// Releases every row at once: what a host does when it stops driving the loop, so no pin
/// survives into the next run. `1` on success, `0` only for a session that is not live.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_unpin_all(session: u32) -> u32 {
    answered(session::unpin_all(session), 1)
}

/// The address of one position column: `axis` `0` for `x`, `1` for `y` — the two words
/// [`session::column`] resolves, and the only two it accepts. `0` on a session that is not
/// live or an axis that is neither — the same `(ptr, len)` convention as `gm_column_ptr`, so a
/// host reads both kinds of view one way.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_column_ptr(session: u32, axis: u32) -> u32 {
    match session::column(session, axis, true) {
        Ok(address) => {
            errors::clear();
            address
        }
        Err(code) => refuse(code, 0),
    }
}

/// The element count of one position column: one per node, `axis` `0` for `x`, `1` for `y`.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_column_len(session: u32, axis: u32) -> u32 {
    match session::column(session, axis, false) {
        Ok(len) => {
            errors::clear();
            len
        }
        Err(code) => refuse(code, 0),
    }
}

/// Releases the session. Its id is never reissued (C6), so a stale id reads `InvalidSession`
/// and never another session's positions. `1` on success, `0` on any refusal — unlike
/// `gm_release`, which returns nothing: a host that ignored this one would leak a session per
/// graph without being able to tell.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_release(session: u32) -> u32 {
    answered(session::release(session), 1)
}

/// The parameter set a `(ptr, len)` pair carries: the compiled-in defaults for `len == 0`, or
/// thirteen `f64`s from a live `gm_alloc` allocation (C5).
///
/// **A buffer is checked with `is_live` before it is read, and `params_len == 0` is not read
/// at all** — an absent buffer carries no bytes, so `params_ptr` means nothing there and must
/// not be dereferenced to find that out. `gm_run` makes the same call about its own unused
/// `params_ptr`.
fn read_params(ptr: u32, len: u32) -> Result<LiveParams, Code> {
    if len == 0 {
        return Ok(LiveParams::default());
    }
    if len as usize != session::PARAMS_LEN || !is_live(ptr, len) {
        return Err(Code::SessionParamsInvalid);
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns, so the bytes are readable for the length checked immediately above;
    // the buffer outlives this call (only the caller's own later `gm_free` frees it), and
    // `decode` copies the fields out rather than retaining the slice.
    let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) };
    params::decode(bytes).ok_or(Code::SessionParamsInvalid)
}

/// `word` on success, `0` on a refusal, with the code recorded either way (C4): the shape
/// every `1`-means-ok export in this ABI shares.
pub(super) fn answered(outcome: Result<(), Code>, word: u32) -> u32 {
    match outcome {
        Ok(()) => {
            errors::clear();
            word
        }
        Err(code) => refuse(code, 0),
    }
}

/// Records `code` as the reason for returning `word`, and hands `word` back — one function, so
/// no export can return a refusal without saying why.
pub(super) fn refuse(code: Code, word: u32) -> u32 {
    errors::set(code);
    word
}
