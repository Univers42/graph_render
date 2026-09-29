//! The ABI's error channel (`docs/contract/wasm-abi.md` "Errors", C4): `0` is the
//! generic failure return for a handle or a pointer, and it is genuinely ambiguous —
//! `gm_node_count` on a bad handle and on a freshly built empty graph both read `0`.
//! [`gm_last_error`](crate::gm_last_error) resolves it: every fallible export sets this
//! thread-local to [`Code::None`] on success and to the specific reason on failure,
//! last write wins, so the code always describes the most recent call.
//!
//! Target-independent: nothing here touches wasm memory, so it is unit-tested natively.

use std::cell::Cell;

/// Why an export returned its failure sentinel (`0`, or [`Code::None`] for absent id
/// past the allocated set — see `views::column`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Code {
    /// The previous call succeeded, or none has run yet.
    None = 0,
    /// The handle does not name a live graph (never issued, or already released).
    InvalidHandle = 1,
    /// `gm_alloc` could not reserve the requested bytes (`try_reserve` failed).
    AllocFailed = 2,
    /// `gm_free`/`gm_build`'s `(ptr, len)` is not exactly a live `gm_alloc` allocation.
    FreeRefused = 3,
    /// The ingest buffer failed the provisional JSON contract (`ingest` module).
    IngestInvalid = 4,
    /// `gm_run`'s `layout_id` is not `< gm_layout_count()`.
    UnknownLayoutId = 5,
    /// `gm_run`'s `params_len` was not `0` (registry runs take no parameters, C2).
    ParamsMustBeEmpty = 6,
    /// Every `u32` handle id has been issued in this instance; none can be reused (C6).
    HandlesExhausted = 7,
    /// The registered layout returned a `StageError` for this topology.
    LayoutFailed = 8,
    /// A column read back NaN or infinite: a view aliasing this handle's buffers wrote
    /// through it since the last run (D9, C8).
    TamperedGeometry = 9,
    /// The handle has no geometry yet: `gm_run` has not succeeded on it.
    NoGeometryYet = 10,
    /// `gm_build`'s `(ptr, len)` is not exactly a live `gm_alloc` allocation.
    BuildSourceInvalid = 11,
    /// An index argument (e.g. `gm_layout_id`) is past the end of its list.
    IndexOutOfRange = 12,
}

thread_local! {
    static LAST: Cell<Code> = const { Cell::new(Code::None) };
}

/// Records `code` as the outcome of the export now returning.
pub fn set(code: Code) {
    LAST.with(|cell| cell.set(code));
}

/// Records success — every export's non-error return path calls this, so a stale code
/// from an earlier call never survives a later success (C4).
pub fn clear() {
    set(Code::None);
}

/// The last recorded code, as the wire's `u32`. `gm_last_error`'s body.
pub fn get() -> u32 {
    LAST.with(Cell::get) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_write_wins_and_starts_at_none() {
        // Cleared explicitly: the thread-local is process-wide across tests.
        clear();
        assert_eq!(get(), Code::None as u32);
        set(Code::InvalidHandle);
        assert_eq!(get(), Code::InvalidHandle as u32);
        set(Code::AllocFailed);
        assert_eq!(get(), Code::AllocFailed as u32, "the more recent call wins");
        clear();
        assert_eq!(get(), 0, "clear is the same as recording None");
    }

    #[test]
    fn every_code_is_distinct_and_none_is_the_wires_zero() {
        let codes = [
            Code::None,
            Code::InvalidHandle,
            Code::AllocFailed,
            Code::FreeRefused,
            Code::IngestInvalid,
            Code::UnknownLayoutId,
            Code::ParamsMustBeEmpty,
            Code::HandlesExhausted,
            Code::LayoutFailed,
            Code::TamperedGeometry,
            Code::NoGeometryYet,
            Code::BuildSourceInvalid,
            Code::IndexOutOfRange,
        ];
        let mut values: Vec<u32> = codes.iter().map(|&c| c as u32).collect();
        values.sort_unstable();
        values.dedup();
        assert_eq!(
            values.len(),
            codes.len(),
            "every Code has its own wire value"
        );
        assert_eq!(
            Code::None as u32,
            0,
            "0 is both a data value and a code: ambiguous"
        );
    }
}
