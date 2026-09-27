//! graph-wasm — thin `extern "C"` glue over graph-core, for the browser and the Node
//! hash harness. No wasm-bindgen (`prompt.md` §3.2): every export takes and returns
//! plain numbers. A buffer comes back as a pointer to `[len: u32 LE][len bytes]`,
//! valid until the next export call.
//!
//! `gm_synthetic` (Phase 0) and `gm_topology` (Phase 1) are the hash gate's stages. With the `probe`
//! feature it also exports `gm_probe`, which carries the D1 measurement to wasm32 so it
//! can be compared bit for bit against the same code run natively; the shipped module
//! is built without it, so a measurement instrument never reaches the browser.

#[cfg(target_arch = "wasm32")]
mod exports {
    use std::cell::RefCell;

    thread_local! {
        static OUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    }

    /// Frames `bytes` into the shared out-buffer and returns its address; 0 on failure.
    fn publish(bytes: Option<Vec<u8>>) -> u32 {
        let Some(bytes) = bytes else { return 0 };
        let Ok(len) = u32::try_from(bytes.len()) else {
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

    /// The Phase-0 synthetic snapshot for `seed`, at the compiled-in reference degree.
    // SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in
    // the module is named `gm_synthetic`, so the export cannot collide.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_synthetic(seed: u32) -> u32 {
        publish(graph_core::synthetic_snapshot(seed, graph_core::REFERENCE_DEGREE).ok())
    }

    /// The Phase-1 topology stage for `seed` (`graph_core::topology_stage`).
    // SAFETY: as above — `gm_topology` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_topology(seed: u32) -> u32 {
        publish(graph_core::topology_stage(seed, graph_core::REFERENCE_DEGREE).ok())
    }

    /// The D1 probe buffer (see [`crate::probe`]). Only in the `probe` build.
    // SAFETY: as above — `gm_probe` is the only symbol with this name.
    #[cfg(feature = "probe")]
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_probe() -> u32 {
        publish(Some(crate::probe::probe_bytes()))
    }
}

pub mod probe;
