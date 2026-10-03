//! The two halves of a parameter-carrying run: what a layout publishes
//! (`gm_layout_params`) and the buffer that run takes (`gm_run`'s `params_ptr`/
//! `params_len`, read here). Split out of `build.rs` by the house's 300-line limit —
//! both are one concern, the parameter ABI, and neither is graph lifecycle.
//!
//! `docs/decisions/layout-params.md` is the decision; `docs/contract/wasm-abi.md` is the
//! wire.

use super::LAYOUTS;
use crate::alloc::is_live;
use crate::errors::{self, Code};
use crate::wire::publish;
use graph_core::registry::Capability;

/// The parameters registry layout `i` publishes, framed; `0` with
/// [`Code::IndexOutOfRange`] past the end.
///
/// One export for the whole schema, because a caller resolves a name to an index once
/// (`gm_layout_id`) and then needs the *list*, not a count and a per-index fetch — the
/// same reason `gm_layout_id` publishes a whole id rather than a byte at a time. A layout
/// that publishes nothing answers with `param_count = 0`, which is an answer and not a
/// refusal: "this layout takes no parameters" is something a caller has to be able to
/// read.
///
/// The body is little-endian (`graph_contract::params::ParamsView::encode`): a `u32`
/// count, then per parameter a `u32` name length and its UTF-8 bytes, a `u8` kind tag,
/// four `f64`s (`min`, `max`, `default`, `step`) and a `u32` doc length with its bytes.
/// The order is the buffer order, so index `i` of one is index `i` of the other.
// SAFETY: as `gm_layout_count`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_layout_params(layout_index: u32) -> u32 {
    match LAYOUTS.get(layout_index as usize) {
        Some(layout) => {
            errors::clear();
            publish(layout.params().encode())
        }
        None => {
            errors::set(Code::IndexOutOfRange);
            0
        }
    }
}

/// The run's parameter buffer as a slice this call owns, or the code it is refused with.
///
/// Empty is not an error and reads nothing: it means the layout's own defaults, which is
/// what every pre-ABI-2 caller sends. The `(ptr, len)` is checked live *before* the slice
/// is formed, so a pointer the caller never allocated is a refusal and not a trap
/// (`is_live`, as `exports/session.rs` does for the force session's own buffer).
pub(super) fn read_params<'a>(layout: &Capability, ptr: u32, len: u32) -> Result<&'a [u8], Code> {
    if len == 0 {
        return Ok(&[]);
    }
    let view = layout.params();
    if view.is_empty() {
        return Err(Code::ParamsNotAccepted);
    }
    let expected = u32::try_from(view.buffer_len()).map_err(|_| Code::ParamsMalformed)?;
    if len != expected {
        return Err(Code::ParamsMalformed);
    }
    if !is_live(ptr, len) {
        return Err(Code::ParamsMalformed);
    }
    // SAFETY: `is_live` confirmed this exact `(ptr, len)` is a `gm_alloc` allocation the
    // caller still owns; the buffer outlives this whole call (freed only by the caller's
    // own later `gm_free`), so borrowing it for the duration of `run_params` is sound,
    // and nothing here retains the slice past this function. The length matches the
    // layout's own buffer length, and every `f64` inside it is read a byte at a time
    // (`ParamsView::value`), because an 8-byte load at a 4-aligned address traps on
    // wasm32 (`crate::alloc::ALIGN`).
    Ok(unsafe { std::slice::from_raw_parts(ptr as *const u8, len as usize) })
}
