//! graph-wasm — thin `extern "C"` glue over graph-core, for the browser and the Node
//! hash harness. No wasm-bindgen (`prompt.md` §3.2): every export takes and returns
//! plain numbers. A buffer comes back as a pointer to `[len: u32 LE][len bytes]`,
//! valid until the next export call.
//!
//! Two ways to build a handle, and that is not an accident: [`exports::gm_build`] reads
//! the **provisional** node/edge JSON (C13) and is unchanged; [`exports::gm_build_contract`]
//! reads the phase-10 **ingest contract** (`docs/contract/ingest-schema.json`) and derives
//! the graph through `graph_core::ingest`'s single derivation. Each reader refuses the other
//! format's document, so neither can quietly drift into the other's meaning — see
//! `docs/contract/wasm-abi.md` "Two build paths".
//!
//! `gm_topology` and one `gm_layout_*` export per registered layout are the hash gate's
//! stages: each runs the pipeline over the gate's model for a seed and returns its own
//! stage's bytes, always at the compiled-in defaults (the wasm arm never sees a negative
//! control's mutation — `hashgate.rs`'s [`Knob`]s perturb the native arm only, so a wired
//! one surfaces as exactly the divergence against this honest wasm baseline). With the
//! `probe` feature it also exports `gm_probe`, which carries the D1 measurement to wasm32
//! so it can be compared bit for bit against the same code run natively; the shipped
//! module is built without it, so a measurement instrument never reaches the browser.
//!
//! **`gm_force_session_*` is the other surface**: the live force session, the one a host
//! drives tick by tick rather than in one `gm_run`. Its table of record is
//! `docs/decisions/force-wasm-abi.md`; `crate::session` holds the physics-free logic and
//! `crate::exports::session` the frames over it.

#[cfg(target_arch = "wasm32")]
mod gate_exports {
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
        publish(
            pipeline(seed, graph_core::layout::tidy_tree::ID).map(|run| run.snapshot.to_bytes()),
        )
    }

    /// The `layout.treemap.squarified` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_treemap_squarified` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_treemap_squarified(seed: u32) -> u32 {
        publish(pipeline(seed, graph_core::layout::treemap::ID).map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.circular.radial` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_circular_radial` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_circular_radial(seed: u32) -> u32 {
        publish(pipeline(seed, graph_core::layout::circular::ID).map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.packing.circle` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_packing_circle` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_packing_circle(seed: u32) -> u32 {
        publish(
            pipeline(seed, graph_core::layout::circle_packing::ID)
                .map(|run| run.snapshot.to_bytes()),
        )
    }

    /// The `layout.spectral` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_spectral` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_spectral(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.spectral").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.mds.pivot` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_mds_pivot` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_mds_pivot(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.mds.pivot").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.force.barnes_hut` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_force_barnes_hut` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_force_barnes_hut(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.force.barnes_hut").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.forceatlas2` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_forceatlas2` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_forceatlas2(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.forceatlas2").map(|run| run.snapshot.to_bytes()))
    }

    /// The `layout.dag.sugiyama` stage's snapshot bytes for `seed`.
    // SAFETY: as above — `gm_layout_dag_sugiyama` is the only symbol with this name.
    #[unsafe(no_mangle)]
    pub extern "C" fn gm_layout_dag_sugiyama(seed: u32) -> u32 {
        publish(pipeline(seed, "layout.dag.sugiyama").map(|run| run.snapshot.to_bytes()))
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

// C21: everything below is target-independent and unit-tested natively (`cargo test`,
// no wasm32 target needed) — but its only *non-test* caller is `exports.rs`, which is
// itself wasm32-only. Gated on `any(test, target_arch = "wasm32")` so a plain native
// `cargo build`/`clippy` (neither test nor wasm32) does not compile modules it cannot
// call, which is what a `-D warnings` dead-code lint would otherwise catch on that one
// build; `cargo test` and the wasm32 release build both still get the real thing.
#[cfg(any(test, target_arch = "wasm32"))]
mod alloc;
pub mod analysis;
#[cfg(any(test, target_arch = "wasm32"))]
mod contract;
#[cfg(any(test, target_arch = "wasm32"))]
mod errors;
mod exports;
#[cfg(any(test, target_arch = "wasm32"))]
mod handle;
#[cfg(any(test, target_arch = "wasm32"))]
mod ingest;
mod memory_measure;
#[cfg(any(test, all(feature = "threads", target_arch = "wasm32")))]
mod pool;
pub mod post;
#[cfg(any(test, target_arch = "wasm32"))]
mod seed_ingest;
#[cfg(any(test, target_arch = "wasm32"))]
mod session;
#[cfg(any(test, target_arch = "wasm32"))]
mod stage_exports;
#[cfg(any(test, target_arch = "wasm32"))]
pub(crate) mod views;
