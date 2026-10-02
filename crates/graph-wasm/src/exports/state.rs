//! State shared by every export in `super`: the live handle table and the one
//! out-buffer framed return values are published into. One copy of each, so a value
//! `build.rs` inserts is exactly what `columns.rs` later reads back.

use crate::errors::{self, Code};
use crate::handle::Handles;
use std::cell::RefCell;

thread_local! {
/// The live handle table, shared with the `replicas` feature's `gm_run_replica`: a
/// replicated run is a run *of a handle*, so it must be the same handle, in the same
/// table, with the same snapshot face — otherwise a replica's result could not be read
/// back through `gm_snapshot_bytes` and the whole comparison would need a second ABI.
/// `crate`-visible rather than `pub(super)` for exactly that one extra reader.
pub(crate) static HANDLES: RefCell<Handles> = const { RefCell::new(Handles::new()) };
static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Frames `bytes` as `[len: u32 LE][len bytes]` in the shared out-buffer and returns its
/// address. The motor owns this buffer; it is valid until the next motor call (C7).
/// `crate`-visible for `crate::replica` too: a framed return from a replicated export must
/// land in the *same* buffer, or the host would have two framed faces and no rule for which.
pub(crate) fn publish(bytes: Vec<u8>) -> u32 {
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
