//! The POST and ANALYSIS stages over the ABI (`docs/contract/wasm-abi.md` "POST" and
//! "ANALYSIS"): `gm_post_count`/`gm_post_id`/`gm_post_run` and
//! `gm_analysis_count`/`gm_analysis_id`/`gm_analysis_run`.
//!
//! Each export is a thin delegation to [`crate::stage_exports`], which holds the work and
//! the refusal table and is unit-tested natively (C21) — so every branch of every export
//! below is pinned without a wasm build in the loop. What lives here is only what needs
//! the real target: the handle table, the out-buffer, and the framed return.

use super::state::{HANDLES, publish};
use crate::errors;
use crate::stage_exports;

/// How many POST capabilities the ABI exposes. Registry-driven like `gm_layout_count`
/// (C1): a capability added to `crate::post::CAPABILITIES` changes this with no ABI
/// change.
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this
// crate is named `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_post_count() -> u32 {
    errors::clear();
    crate::post::count()
}

/// The capability id at registry index `i`, framed UTF-8; `0` past the end
/// (`Code::IndexOutOfRange`). `gm_post_run`'s `post_index` *is* this index — the SDK maps
/// a string id to it by scanning `0..gm_post_count()` once, never a hard-coded constant.
// SAFETY: as `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_post_id(i: u32) -> u32 {
    framed_id(stage_exports::post_id(i))
}

/// How many analyses the ABI exposes. Registry-driven like `gm_layout_count` (C1).
// SAFETY: as `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_analysis_count() -> u32 {
    errors::clear();
    crate::analysis::count()
}

/// The analysis id at registry index `i`, framed UTF-8; `0` past the end
/// (`Code::IndexOutOfRange`). `gm_analysis_run`'s `index` *is* this index.
// SAFETY: as `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_analysis_id(i: u32) -> u32 {
    framed_id(stage_exports::analysis_id(i))
}

/// Runs POST capability `post_index` over `handle`'s last successful layout run and
/// **replaces that run's edge geometry** with the result, so the handle's columns
/// (`gm_column_ptr`/`gm_column_len`, and the two snapshot faces) read the new edges.
/// `1` on success, `0` on any refusal, which leaves the handle's geometry as it was.
// SAFETY: as `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_post_run(handle: u32, post_index: u32) -> u32 {
    // `.map(drop)`: the snapshot borrows the table, which must not outlive the closure.
    let ran = HANDLES.with(|handles| {
        stage_exports::post_run(&mut handles.borrow_mut(), handle, post_index).map(drop)
    });
    match ran {
        Ok(()) => {
            errors::clear();
            1
        }
        Err(code) => {
            errors::set(code);
            0
        }
    }
}

/// Runs analysis `index` over `handle`'s topology and returns its canonical JSON face,
/// framed UTF-8; `0` on any refusal. **No geometry is required** — see
/// [`stage_exports::analysis_run`].
// SAFETY: as `gm_post_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_analysis_run(handle: u32, index: u32) -> u32 {
    let ran = HANDLES.with(|handles| stage_exports::analysis_run(&handles.borrow(), handle, index));
    match ran {
        Ok(text) => {
            errors::clear();
            publish(text.into_bytes())
        }
        Err(code) => {
            errors::set(code);
            0
        }
    }
}

/// An id, framed, or the refusal as a `0`: the one shape both `gm_post_id` and
/// `gm_analysis_id` publish, so the two cannot drift in how they report a bad index.
fn framed_id(id: Result<&'static str, errors::Code>) -> u32 {
    match id {
        Ok(id) => {
            errors::clear();
            publish(id.as_bytes().to_vec())
        }
        Err(code) => {
            errors::set(code);
            0
        }
    }
}
