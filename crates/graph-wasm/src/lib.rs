//! graph-wasm — thin `extern "C"` glue over graph-core, for the browser and the Node
//! hash harness. No wasm-bindgen (`prompt.md` §3.2): every export takes and returns
//! plain numbers. A buffer comes back as a pointer to `[len: u32 LE][len bytes]`,
//! valid until the next export call.
//!
//! `gm_topology` and one `gm_layout_*` export per registered layout are the hash gate's
//! stages: each runs the pipeline over the gate's model for a seed and returns its own
//! stage's bytes, always at the compiled-in defaults (the wasm arm never sees a negative
//! control's mutation — `hashgate.rs`'s [`Knob`]s perturb the native arm only, so a wired
//! one surfaces as exactly the divergence against this honest wasm baseline). With the
//! `probe` feature it also exports `gm_probe`, which carries the D1 measurement to wasm32
//! so it can be compared bit for bit against the same code run natively; the shipped
//! module is built without it, so a measurement instrument never reaches the browser.

#[cfg(target_arch = "wasm32")]
mod exports {
    use graph_core::{PipelineRun, REFERENCE_DEGREE, gate_node_count, run_with, seeded_model};
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

    /// The pipeline over the gate's model for `seed`, with the registered layout `id` at
    /// its default parameters and the compiled-in reference degree.
    fn pipeline(seed: u32, id: &str) -> Option<PipelineRun> {
        let layout = graph_core::registry::find(id)?;
        let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
        run_with(&nodes, &edges, layout.id, layout.run).ok()
    }

    /// The topology stage's bytes for `seed`.
    // SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in
    // the module is named `gm_topology`, so the export cannot collide.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_topology(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.grid").map(|run| run.topology))
    }

    /// The `layout.grid` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_grid` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_grid(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.grid").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.tree.tidy` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_tree_tidy` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_tree_tidy(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.tree.tidy").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.treemap.squarified` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_treemap_squarified` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_treemap_squarified(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.treemap.squarified").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.circular.radial` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_circular_radial` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_circular_radial(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.circular.radial").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.packing.circle` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_packing_circle` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_packing_circle(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.packing.circle").map(|run| run.snapshot.to_bytes()))
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
