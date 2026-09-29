//! State shared by every export in `super`: the live handle table and the one
//! out-buffer framed return values are published into. One copy of each, so a value
//! `build.rs` inserts is exactly what `columns.rs` later reads back.

use crate::errors::{self, Code};
use crate::handle::Handles;
use std::cell::RefCell;

thread_local! {
    pub(super) static HANDLES: RefCell<Handles> = RefCell::new(Handles::new());
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Frames `bytes` as `[len: u32 LE][len bytes]` in the shared out-buffer and returns its
/// address. The motor owns this buffer; it is valid until the next motor call (C7).
pub(super) fn publish(bytes: Vec<u8>) -> u32 {
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
