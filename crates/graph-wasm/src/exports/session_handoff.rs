//! The four exports over [`crate::session::handoff`]: the two velocity columns and the simple
//! graph's three, as `(ptr, len)` pairs.
//!
//! # Why a second file and not four more in `session`
//!
//! `session.rs` is at the house's 300-line limit, and these are a separate ABI grouping — the
//! device's tick input, not the renderer's frame — with their own C7 lifetimes to state. Every
//! `#[unsafe(no_mangle)]` function is a real wasm export regardless of which file defines it,
//! so the split is invisible on the wire.

use super::session::refuse;
use crate::errors;
use crate::session::handoff;

/// The address of one velocity column: `axis` `0` for `vx`, `1` for `vy` — the two words
/// [`handoff::velocity`] resolves, and the only two it accepts. `0` on a session that is not
/// live or an axis that is neither — the same `(ptr, len)` convention as
/// `gm_force_session_column_ptr`, so a host reads both kinds of view one way.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_velocity_ptr(session: u32, axis: u32) -> u32 {
    match handoff::velocity(session, axis, true) {
        Ok(address) => {
            errors::clear();
            address
        }
        Err(code) => refuse(code, 0),
    }
}

/// The element count of one velocity column: one per node, `axis` `0` for `vx`, `1` for `vy`.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_velocity_len(session: u32, axis: u32) -> u32 {
    match handoff::velocity(session, axis, false) {
        Ok(len) => {
            errors::clear();
            len
        }
        Err(code) => refuse(code, 0),
    }
}

/// The address of one simple-graph column: `column` `0` for `lo`, `1` for `hi`, `2` for
/// `strength` — the three words [`handoff::edge`] resolves, and the only three it accepts. `0`
/// on a session that is not live or a column that is none of the three.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_edge_ptr(session: u32, column: u32) -> u32 {
    match handoff::edge(session, column, true) {
        Ok(address) => {
            errors::clear();
            address
        }
        Err(code) => refuse(code, 0),
    }
}

/// The element count of the simple graph's columns: the simple edge count `m`, the same for
/// `lo`, `hi` and `strength`, so it takes no `column` argument.
// SAFETY: as `gm_force_session_create`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_force_session_edge_len(session: u32) -> u32 {
    match handoff::edge(session, 0, false) {
        Ok(len) => {
            errors::clear();
            len
        }
        Err(code) => refuse(code, 0),
    }
}
