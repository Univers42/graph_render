//! What crosses the ABI as a `u32`: the one out-buffer every framed return is published
//! into, and the address/length conversion every export refuses through instead of
//! truncating. One copy of each, shared by `crate::exports` and the hash-gate shims in
//! `lib.rs`, so C7's "valid until the next call into the module" holds for both.
//!
//! Target-independent: on the native 64-bit test host every heap address is past `u32`,
//! which is what makes the refusal path reachable in `cargo test`.

use crate::errors::{self, Code};
use std::cell::RefCell;

thread_local! {
    static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Frames `bytes` as `[len: u32 LE][len bytes]` in the shared out-buffer and returns its
/// address. The motor owns this buffer; it is valid until the next motor call (C7). A
/// nonzero return clears the code; a `0` always carries its reason: `AllocFailed` for a
/// body past `u32`, `IndexOutOfRange` for an address the wire cannot carry.
pub fn publish(bytes: Vec<u8>) -> u32 {
    let Ok(len) = u32::try_from(bytes.len()) else {
        errors::set(Code::AllocFailed);
        return 0;
    };
    OUT.with(|cell| {
        let mut out = cell.borrow_mut();
        out.clear();
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&bytes);
        errors::reply(to_wire(out.as_ptr() as usize))
    })
}

/// A hash-gate stage's bytes, published; `None` is the pipeline refusing the seed, read
/// as `LayoutFailed`.
pub fn publish_stage(bytes: Option<Vec<u8>>) -> u32 {
    let Some(bytes) = bytes else {
        errors::set(Code::LayoutFailed);
        return 0;
    };
    publish(bytes)
}

/// A host address or length as the wire's `u32`, refused rather than truncated.
pub fn to_wire(value: usize) -> Result<u32, Code> {
    u32::try_from(value).map_err(|_| Code::IndexOutOfRange)
}

#[cfg(test)]
mod tests;
