//! The live session's other columns: the two velocity columns and the simple graph's three.
//!
//! # Why these are a second pair of calls and not a wider `column`
//!
//! [`column`](super::column) names the two *position* columns, which a renderer reads once per
//! frame. These name the state a *device* needs in order to run the same tick: the velocities
//! the next tick integrates from, and the adjacency it integrates over. Same `(ptr, len)`
//! convention, same C3 empty rule, same C7 lifetime, so a host reads both kinds of view one
//! way — which is the whole reason the four exports mirror `gm_column_ptr`/`gm_column_len`
//! rather than inventing a third shape.
//!
//! # Who moves what
//!
//! A particle-mesh tick swaps the velocity columns with its scratch
//! (`particle_mesh/motion.rs`), exactly as it swaps the position ones, so a velocity address
//! is good until the session's next tick. The simple graph is built once, at creation and at
//! each grow, so an edge address is good until the next grow and no longer — a device may
//! hold an edge view across ticks and re-read it after each.

use crate::errors::Code;
use crate::session::with;
use crate::wire::to_wire;

/// The address (`want_ptr`) or the element count (`want_len`) of one velocity column:
/// `axis` `0` is `vx`, `1` is `vy`.
///
/// The address is the session's own column, good until the session's next
/// `gm_force_session_tick` or `gm_force_session_grow` (C7): a particle-mesh tick swaps it with
/// the tick's scratch, and a grow appends a row per node. An address or length the wire's
/// `u32` cannot carry is refused with [`Code::IndexOutOfRange`], never truncated.
pub fn velocity(id: u32, axis: u32, want_ptr: bool) -> Result<u32, Code> {
    with(id, |session| {
        let values = match axis {
            0 => session.vxs(),
            1 => session.vys(),
            _ => return Err(Code::IndexOutOfRange),
        };
        // C3: an empty column reads (0, 0), never an empty `Vec`'s dangling address.
        if values.is_empty() {
            return Ok(0);
        }
        let address = if want_ptr {
            values.as_ptr() as usize
        } else {
            values.len()
        };
        to_wire(address)
    })
}

/// The address (`want_ptr`) or the element count (`want_len`) of one simple-graph column:
/// `column` `0` is `lo`, `1` is `hi`, `2` is `strength`. All three hold the same `m` simple
/// edges, so the `want_len` answer does not depend on `column`.
///
/// The address is the session's own column, good until the session's next
/// `gm_force_session_grow` (C7): a grow appends edges and may move the storage. A tick does
/// not move it, which is the difference from [`velocity`](Self::velocity) and
/// [`column`](super::column). An address or length the wire's `u32` cannot carry is refused
/// with [`Code::IndexOutOfRange`], never truncated.
pub fn edge(id: u32, column: u32, want_ptr: bool) -> Result<u32, Code> {
    with(id, |session| {
        let (lo, hi, strength) = session.simple_edges();
        let (address, len) = match column {
            0 => (lo.as_ptr() as usize, lo.len()),
            1 => (hi.as_ptr() as usize, hi.len()),
            2 => (strength.as_ptr() as usize, strength.len()),
            _ => return Err(Code::IndexOutOfRange),
        };
        // C3: an empty column reads (0, 0), never an empty `Vec`'s dangling address.
        if len == 0 {
            return Ok(0);
        }
        to_wire(if want_ptr { address } else { len })
    })
}
