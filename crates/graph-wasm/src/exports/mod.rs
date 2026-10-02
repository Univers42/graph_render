//! The ABI's `extern "C"` surface beyond the retained hash-gate shims
//! (`docs/contract/wasm-abi.md` is authoritative; `crate::gate_exports` is the shim
//! module C20 keeps green). Wasm32-only: this is the pointer layer over the
//! target-independent `alloc`, `handle`, `ingest`, `contract`, `views`, `seed_ingest` and
//! `errors` modules (C21) — every one of those is unit-tested natively; only the
//! functions below need the real target to exist at all.
//!
//! Split across five files by the house's 300-line limit, not by any ABI grouping (and
//! `gm_abi_version`, below, the one export that belongs to none of them):
//! [`state`] holds the shared handle table and out-buffer every export below reaches
//! into; [`build`] is graph lifecycle (`gm_build`/`gm_build_contract`/`gm_run`/
//! `gm_node_count`/geometry tags/`gm_last_error`/`gm_seed_ingest`); [`columns`] is
//! reading a finished run's data back out (`gm_column_ptr`/`gm_column_len`/the two
//! snapshot faces/`gm_release`);
//! [`stages`] is the two stages downstream of LAYOUT — POST (`gm_post_*`) and ANALYSIS
//! (`gm_analysis_*`); [`session`] is the live force session (`gm_force_session_*`, over the
//! target-independent `crate::session`). Every `#[unsafe(no_mangle)]` function is a real wasm
//! export regardless of which of the five files defines it — that boundary is invisible on the
//! wire.

#![cfg(target_arch = "wasm32")]

mod build;
mod columns;
mod session;
mod stages;
mod state;

/// [`crate::ABI_VERSION`]: the first call a loader makes, so a module built from another
/// revision than the SDK is refused by number before any call whose meaning moved.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_abi_version`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_abi_version() -> u32 {
    crate::errors::clear();
    crate::ABI_VERSION
}
