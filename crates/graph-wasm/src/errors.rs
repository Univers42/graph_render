//! The ABI's error channel (`docs/contract/wasm-abi.md` "Errors", C4): `0` is the
//! generic failure return for a handle or a pointer, and it is genuinely ambiguous —
//! `gm_node_count` on a bad handle and on a freshly built empty graph both read `0`.
//! [`gm_last_error`](crate::gm_last_error) resolves it: every fallible export sets this
//! thread-local to [`Code::None`] on success and to the specific reason on failure,
//! last write wins, so the code always describes the most recent call.
//!
//! Target-independent: nothing here touches wasm memory, so it is unit-tested natively.

#[cfg(any(test, target_arch = "wasm32"))]
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
    /// An index argument (e.g. `gm_layout_id`, `gm_post_id`, `gm_analysis_id`) is past
    /// the end of its list, or a host address or length does not fit the wire's `u32`
    /// (a column's pointer or element count, refused rather than truncated).
    IndexOutOfRange = 12,
    /// The registered POST capability returned a `StageError` for this geometry, or the
    /// edges it produced did not fit the snapshot.
    PostFailed = 13,
    /// `gm_build_contract`'s buffer failed the phase-10 ingest contract: either the
    /// contract's own strict reader refused the document, or the derivation refused the
    /// graph it describes (a tag value that cannot round-trip through the node-id
    /// grammar, H5). One code for both, deliberately: a caller asking "was my document
    /// accepted" needs one answer, and the two refusals are already distinguishable by
    /// which one is reachable — the reader's checks run first and cover everything it can
    /// see.
    ///
    /// **Not** `IngestInvalid`: that is the *provisional* node/edge JSON's code, and
    /// `gm_build` keeps it. The two formats are different documents with different
    /// meanings, and a code that did not say which one was refused would let a caller
    /// handle a contract rejection as a node/edge rejection.
    ContractInvalid = 14,
    /// The session id does not name a live force session (never issued, or already
    /// released). The force session's own id space: never the graph handle's, which keeps
    /// its own [`Code::InvalidHandle`].
    InvalidSession = 15,
    /// A force session's `(params_ptr, params_len)` is neither `0` (the compiled-in
    /// defaults) nor exactly the parameter buffer's own length, so there is no reading of
    /// it to attempt. Never a silent "use the defaults" for a length it did not recognise.
    SessionParamsInvalid = 16,
    /// The session itself refused: a parameter out of its range (never clamped), a row
    /// past the last node column, or a coordinate that is not finite (D9). One code for
    /// all of them, as graph-core's own `SessionError` is one refusal to the caller — the
    /// field and the rule are in the refusal's text, which this ABI does not carry, so a
    /// host that needs to name the field reads it back from `gm_last_error`'s code plus its
    /// own bounds table.
    SessionRefused = 17,
    /// The analysis ran but its report has no JSON text: a non-finite score or modularity
    /// (D9; `NaN` is not a JSON number), or a column longer than `u32` can count.
    AnalysisFailed = 18,
    /// `gm_build`'s buffer is longer than `ingest::MAX_INGEST_BYTES`, refused before any
    /// parsing. **Not** `IngestInvalid`: that code means the document was read and found
    /// malformed, and a host that got one has a bug in its document while a host that got
    /// this one has a document it must split or shrink. Without it the oversized document
    /// reached wasm32's address-space limit inside an infallible allocation and the host saw
    /// an `unreachable` trap it could not name (F-16).
    IngestTooLarge = 19,
}

impl Code {
    /// The code's name, as the SDK's `CODE_NAMES` and `docs/contract/wasm-abi.md`'s Errors
    /// table spell it. Written out rather than taken from `Debug`, whose text is not a
    /// promise; `mirrors` pins the three to one another.
    pub fn name(&self) -> &'static str {
        match self {
            Self::None => "None",
            Self::InvalidHandle => "InvalidHandle",
            Self::AllocFailed => "AllocFailed",
            Self::FreeRefused => "FreeRefused",
            Self::IngestInvalid => "IngestInvalid",
            Self::UnknownLayoutId => "UnknownLayoutId",
            Self::ParamsMustBeEmpty => "ParamsMustBeEmpty",
            Self::HandlesExhausted => "HandlesExhausted",
            Self::LayoutFailed => "LayoutFailed",
            Self::TamperedGeometry => "TamperedGeometry",
            Self::NoGeometryYet => "NoGeometryYet",
            Self::BuildSourceInvalid => "BuildSourceInvalid",
            Self::IndexOutOfRange => "IndexOutOfRange",
            Self::PostFailed => "PostFailed",
            Self::ContractInvalid => "ContractInvalid",
            Self::InvalidSession => "InvalidSession",
            Self::SessionParamsInvalid => "SessionParamsInvalid",
            Self::SessionRefused => "SessionRefused",
            Self::AnalysisFailed => "AnalysisFailed",
            Self::IngestTooLarge => "IngestTooLarge",
        }
    }
}

// The thread-local channel is the exports' alone: a native caller reads the `Result`.
#[cfg(any(test, target_arch = "wasm32"))]
thread_local! {
    static LAST: Cell<Code> = const { Cell::new(Code::None) };
}

/// Records `code` as the outcome of the export now returning.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn set(code: Code) {
    LAST.with(|cell| cell.set(code));
}

/// Records success — every export's non-error return path calls this, so a stale code
/// from an earlier call never survives a later success (C4).
#[cfg(any(test, target_arch = "wasm32"))]
pub fn clear() {
    set(Code::None);
}

/// The last recorded code, as the wire's `u32`. `gm_last_error`'s body.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn get() -> u32 {
    LAST.with(Cell::get) as u32
}

/// An export's answer: the value on success with the code cleared, `0` with the reason
/// recorded on a refusal — so no `0` that is a refusal leaves a stale code behind.
#[cfg(any(test, target_arch = "wasm32"))]
pub fn reply(result: Result<u32, Code>) -> u32 {
    match result {
        Ok(value) => {
            clear();
            value
        }
        Err(code) => {
            set(code);
            0
        }
    }
}

#[cfg(test)]
mod mirrors;

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
            Code::PostFailed,
            Code::ContractInvalid,
            Code::InvalidSession,
            Code::SessionParamsInvalid,
            Code::SessionRefused,
            Code::AnalysisFailed,
            Code::IngestTooLarge,
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

    /// Append-only, and this is what makes that a promise rather than a hope: a code
    /// added in the middle would renumber every later one, and `CODE_NAMES` in
    /// `crates/graph-sdk-js/src/errors.ts` indexes the same order — an ABI rename that
    /// silently turned a caller's `UnknownLayoutId` into a `NoGeometryYet`.
    #[test]
    fn the_new_code_appends_and_does_not_move_any_other() {
        assert_eq!(
            [
                Code::None as u32,
                Code::InvalidHandle as u32,
                Code::AllocFailed as u32,
                Code::FreeRefused as u32,
                Code::IngestInvalid as u32,
                Code::UnknownLayoutId as u32,
                Code::ParamsMustBeEmpty as u32,
                Code::HandlesExhausted as u32,
                Code::LayoutFailed as u32,
                Code::TamperedGeometry as u32,
                Code::NoGeometryYet as u32,
                Code::BuildSourceInvalid as u32,
                Code::IndexOutOfRange as u32,
                Code::PostFailed as u32,
                Code::ContractInvalid as u32,
                Code::InvalidSession as u32,
                Code::SessionParamsInvalid as u32,
                Code::SessionRefused as u32,
                Code::AnalysisFailed as u32,
                Code::IngestTooLarge as u32,
            ],
            [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19,
            ],
            "every code keeps the wire value it already had"
        );
        assert_ne!(
            Code::InvalidSession as u32,
            Code::InvalidHandle as u32,
            "a dead session must not read as a dead graph handle, and the other way round"
        );
        assert_ne!(
            Code::ContractInvalid as u32,
            Code::IngestInvalid as u32,
            "a refused contract document must not read as a refused node/edge one"
        );
    }
}
