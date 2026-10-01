//! The **live force session** behind the wasm ABI (`docs/decisions/force-wasm-abi.md`):
//! graph-core's [`ForceSession`] behind a table of ids, so a host can create one over a
//! graph handle, tick it, pin a node, read its positions, and release it.
//!
//! # Why the physics is not here
//!
//! Every function below is a *naming* of `ForceSession`'s own methods, plus the id table
//! around them. The tick is `ForceSession::step`, the pins are `ForceSession::pin`, and the
//! positions are `ForceSession::xs`/`ys` — one tick, one pin, one column, exactly as
//! `docs/decisions/live-force-session.md` decided. Nothing in this module changes a number,
//! which is what `mod tests`' `the_wasm_facing_functions_reach_the_same_bits_as_step` is
//! there to keep honest: N ticks through these functions are the same bits as
//! `ForceSession::step(N)` called directly.
//! # Target-independent (C21)
//!
//! No wasm pointer and no export appears here. The `#[unsafe(no_mangle)]` layer in
//! `crate::exports::session` is a thin frame over these, so `cargo test` on the host
//! exercises every branch of the session, the table and the parameter wire format with no
//! wasm build in the loop.
//!
//! # Who owns what
//!
//! The session owns its columns; the id is this module's table's, monotonic and never
//! reissued, released by [`release`]. `column` hands out the *address* of the session's own
//! `Vec<f64>` — which is why the value lives behind a `Box` in [`Table`], so no later
//! insert can move it, and why there is no path here that resizes it (see the column
//! invariant test).

/// The wire format itself, reachable as `crate::session::params` because the export layer
/// encodes and decodes the buffer — everything else here is the table and the naming.
pub(crate) mod params;

#[cfg(test)]
mod tests;

/// The parameter buffer's byte length: one `f64` per [`LiveParams`] field, little-endian.
/// Re-exported from `params` because it is the number the wire's two parameter calls are
/// checked against, and the one a host's own buffer has to match.
pub use params::LEN as PARAMS_LEN;

use graph_core::Topology;
use graph_core::layout::force::{ForceSession, LiveParams, NodeRow};
use std::cell::RefCell;

use crate::errors::Code;
use crate::handle::Table;

/// What one tick did, as the wire's status word (`gm_force_session_tick`'s return).
///
/// Three values, not a bool: `0` is the refusal sentinel every export returns, and a
/// caller that read `1` as "ran" and `0` as "refused" could not tell a settled layout from
/// a hot one — which is the one distinction an interactive loop is driving on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Status {
    /// The ticks ran and the layout is still cooling.
    Running = 1,
    /// The ticks ran and the layout has settled (`alpha < alpha_min`, no target holding
    /// it up). Another tick now scales every force by an alpha too small to see.
    Settled = 2,
}

impl Status {
    /// The wire's word for this verdict.
    pub const fn as_u32(self) -> u32 {
        self as u32
    }

    /// The verdict `StepReport::settled` names.
    pub const fn of(settled: bool) -> Self {
        if settled {
            Self::Settled
        } else {
            Self::Running
        }
    }
}

thread_local! {
    /// Live sessions, keyed by the id `gm_force_session_create` returned. One copy per
    /// thread, like `exports::state`'s `HANDLES`: wasm is single-threaded here, and a
    /// `RefCell` borrow panic is the honest failure for a reentrant call rather than a lock
    /// that silently serialises two halves of one tick.
    static SESSIONS: RefCell<Table<ForceSession>> = const { RefCell::new(Table::new()) };
}

/// A session over `topology` at `params`, and the id it answers to.
///
/// Refused with [`Code::SessionRefused`] when a parameter is out of range — never clamped,
/// never dropped (`docs/decisions/live-force-session.md`), and never created half-set: a
/// session that exists is a session whose parameters it will accept.
pub fn create(topology: &Topology, params: LiveParams) -> Result<u32, Code> {
    let session = ForceSession::new(topology, params).map_err(|_| Code::SessionRefused)?;
    SESSIONS
        .with(|live| live.borrow_mut().insert(session))
        .ok_or(Code::HandlesExhausted)
}

/// Runs `ticks` ticks and says whether the layout has settled.
///
/// **The tick count is the only argument that changes the result**: `tick(112)`, `112` calls
/// with `1` and `7` with `16` are the same bytes, which is graph-core's own guarantee and
/// the reason this module has no schedule of its own to get wrong.
pub fn tick(id: u32, ticks: u32) -> Result<Status, Code> {
    with_mut(id, |session| Ok(Status::of(session.step(ticks).settled)))
}

/// The cooling schedule's current value — `f64` on the wire, the one non-`u32` return in
/// this ABI besides the pins' arguments, and deliberately exact rather than framed.
pub fn alpha(id: u32) -> Result<f64, Code> {
    with(id, |session| Ok(session.alpha()))
}

/// The parameters in force, as the wire's `f64` buffer.
pub fn params_of(id: u32) -> Result<LiveParams, Code> {
    with(id, |session| Ok(session.params()))
}

/// Replaces the parameters, leaving the session exactly as it was on any refusal.
pub fn set_params(id: u32, params: LiveParams) -> Result<(), Code> {
    with_mut(id, |session| {
        session.set_params(params).map_err(|_| Code::SessionRefused)
    })
}

/// Sets `alpha` outright — d3's own `alpha(_)`, the verb behind "the user moved something,
/// run it again". Taken exactly and never clamped; `0..=1` or refused.
pub fn reheat(id: u32, alpha: f64) -> Result<(), Code> {
    with_mut(id, |session| {
        session.reheat(alpha).map_err(|_| Code::SessionRefused)
    })
}

/// Holds `row` at `(x, y)` exactly from the next tick on. **This is the drag verb**: the
/// studio's `force.drag` and its `force.pin` are the same call with a different name at the
/// call site, and graph-core has one of them — d3's `node.fx`/`node.fy`.
///
/// `row` is the node's row in the two position columns, i.e. the same dense order
/// `gm_column_ptr`'s `NodeX` addresses. No node-id string crosses this ABI.
pub fn pin(id: u32, row: u32, x: f64, y: f64) -> Result<(), Code> {
    with_mut(id, |session| {
        session
            .pin(NodeRow::new(row), x, y)
            .map_err(|_| Code::SessionRefused)
    })
}

/// Moves every row to `(xs, ys)` — the picture a host is drawing — with velocities zeroed;
/// pins and alpha are kept. Widened from the snapshot's `f32` exactly (every `f32` is an
/// `f64`). Refused when a column's length is not the node count or a value is not finite.
pub fn seat(id: u32, xs: &[f32], ys: &[f32]) -> Result<(), Code> {
    let widen = |column: &[f32]| column.iter().map(|&v| f64::from(v)).collect::<Vec<f64>>();
    with_mut(id, |session| {
        session
            .set_positions(&widen(xs), &widen(ys))
            .map_err(|_| Code::SessionRefused)
    })
}

/// Moves every row back to the spiral a new session starts on, at rest, at the starting
/// alpha; parameters and pins are kept. Fails only for a session that is not live.
pub fn restart(id: u32) -> Result<(), Code> {
    with_mut(id, |session| {
        session.restart();
        Ok(())
    })
}

/// Releases one row, which then integrates again from rest.
pub fn unpin(id: u32, row: u32) -> Result<(), Code> {
    with_mut(id, |session| {
        session
            .unpin(NodeRow::new(row))
            .map_err(|_| Code::SessionRefused)
    })
}

/// Releases every row. Cannot fail for a reason a caller can act on, and refuses a dead
/// session like every other verb: "unpin everything" on an id that never existed is still a
/// caller's mistake worth reporting.
pub fn unpin_all(id: u32) -> Result<(), Code> {
    with_mut(id, |session| {
        session.unpin_all();
        Ok(())
    })
}

/// The address (`want_ptr`) or the element count (`want_len`) of one position column:
/// `axis` `0` is `x`, `1` is `y`. The two calls mirror `gm_column_ptr`/`gm_column_len`'s
/// shape rather than inventing a third convention.
///
/// The address is the session's own column, which **no path in this ABI resizes**, so it
/// stays valid for the session's whole life rather than only until the next call (C7) —
/// [`seat`] copies into the columns in place and never resizes them. A host still
/// treats a view as good only until the next motor call, because a wasm memory growth
/// detaches its `ArrayBuffer`; that is the JS side's hazard, not this address's. An address
/// or length the wire's `u32` cannot carry is refused with [`Code::IndexOutOfRange`], never
/// truncated.
pub fn column(id: u32, axis: u32, want_ptr: bool) -> Result<u32, Code> {
    with(id, |session| {
        let values = match axis {
            0 => session.xs(),
            1 => session.ys(),
            _ => return Err(Code::IndexOutOfRange),
        };
        let address = if want_ptr {
            values.as_ptr() as usize
        } else {
            values.len()
        };
        to_wire(address)
    })
}

/// Releases the session. Its id is never reissued (C6), so a stale id reads
/// [`Code::InvalidSession`] rather than another session's positions.
pub fn release(id: u32) -> Result<(), Code> {
    SESSIONS
        .with(|live| live.borrow_mut().remove(id))
        .map(|_| ())
        .ok_or(Code::InvalidSession)
}

fn with<T>(id: u32, read: impl FnOnce(&ForceSession) -> Result<T, Code>) -> Result<T, Code> {
    SESSIONS.with(|live| {
        let live = live.borrow();
        let session = live.get(id).ok_or(Code::InvalidSession)?;
        read(session)
    })
}

fn with_mut<T>(
    id: u32,
    write: impl FnOnce(&mut ForceSession) -> Result<T, Code>,
) -> Result<T, Code> {
    SESSIONS.with(|live| {
        let mut live = live.borrow_mut();
        let session = live.get_mut(id).ok_or(Code::InvalidSession)?;
        write(session)
    })
}

/// The wire's `u32` for a host address or length, refused when it does not fit.
fn to_wire(address: usize) -> Result<u32, Code> {
    u32::try_from(address).map_err(|_| Code::IndexOutOfRange)
}

/// Drops every live session. Test-only, for the same reason `errors::clear` exists: the
/// table is process-wide across a test binary, so a test that counts ids must start from a
/// known one rather than from whatever ran beside it.
#[cfg(test)]
pub fn reset() {
    SESSIONS.with(|live| {
        let mut live = live.borrow_mut();
        for id in 1..u32::from(u8::MAX) {
            live.remove(id);
        }
    });
}
