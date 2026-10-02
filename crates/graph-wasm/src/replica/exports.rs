//! The three exports model (b) adds on top of the ordinary ABI: `gm_seed_ingest_n` (a model
//! of any size, so a bench can go past the hash gate's 601 nodes), `gm_run_replica` (`gm_run`
//! with a [`Replica`] behind the runner) and nothing else. The handle, the snapshot face and
//! the error code are the ordinary ABI's, deliberately: a replicated run must be
//! indistinguishable from a serial one to everything downstream of the export, or the
//! comparison this model exists to make would need its own column reader.
//!
//! All three are wasm32-only and behind the `replicas` feature, so the shipped module still
//! imports nothing and exports nothing extra.

use super::Replica;
use crate::errors::{self, Code};
use crate::exports::state::{HANDLES, publish};
use crate::handle::Handle;
use crate::seed_ingest;
use graph_contract::binary::Snapshot;
use graph_core::layout::Geometry;
use graph_core::layout::force::{BarnesHut, ForceParams, ParticleMesh};
use graph_core::registry::LAYOUTS;
use graph_core::{REFERENCE_DEGREE, Stage, seeded_model};

/// The gate's model at `seed` but with `n` nodes instead of `2 + seed % 600`, as the
/// provisional ingest JSON `gm_build` reads. Bench-only, and additive: [`gm_seed_ingest`]
/// is untouched, so `harness/wasm-run.mjs`'s hash mode keeps hashing the model it always
/// hashed and the two arms' rows stay comparable.
///
/// The reference degree is the gate's own [`REFERENCE_DEGREE`], not a new constant: the
/// model has to be the same *shape* as the gate's, or "the threads changed nothing" would be
/// a claim about a different graph.
/// `0` on a refusal (an oversize buffer).
// SAFETY: `no_mangle` exports this symbol under its Rust name; no other symbol in this crate
// is named `gm_seed_ingest_n`, so the export cannot collide.
#[unsafe(no_mangle)]
pub extern "C" fn gm_seed_ingest_n(seed: u32, n: u32) -> u32 {
    errors::clear();
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    publish(seed_ingest::document(&nodes, &edges).into_bytes())
}

/// [`gm_run`](crate::exports::gm_run) with a [`Replica`] behind the runner: the same handle,
/// the same registry index, the same snapshot face, the same refusals — only the schedule of
/// the three gathered passes differs. `1` on success, `0` on any refusal.
///
/// Two refusals are this export's own and land before the handle is touched:
/// `ranks == 0` and `rank >= ranks`. There is no instance to run and no instance that is
/// *this* one, and both are a host that miscounted its own workers; answering with a layout
/// anyway would be a run whose bytes no host could predict. `Code::IndexOutOfRange`, the same
/// code every out-of-range index argument in this ABI already uses.
///
/// A rank count above the node count is **not** a refusal: [`partition`] yields
/// `min(len, ranks)` ranges and the surplus instances gather an empty span, which is the
/// serial answer for those nodes.
// SAFETY: as `gm_seed_ingest_n`.
#[unsafe(no_mangle)]
pub extern "C" fn gm_run_replica(handle: u32, layout_id: u32, rank: u32, ranks: u32) -> u32 {
    if ranks == 0 || rank >= ranks {
        errors::set(Code::IndexOutOfRange);
        return 0;
    }
    HANDLES.with(|handles| {
        let mut handles = handles.borrow_mut();
        let Some(entry) = handles.get_mut(handle) else {
            errors::set(Code::InvalidHandle);
            return 0;
        };
        // Every refusal clears the run's geometry first, exactly as `gm_run` does (C4): a
        // failed replicated run must never leave the serial run's snapshot to be served.
        entry.snapshot = None;
        entry.geometry = None;
        let replica = Replica::new(rank, ranks);
        match run_force(entry, layout_id, &replica).and_then(|geometry| paired(entry, geometry)) {
            Ok((geometry, snapshot)) => {
                entry.geometry = Some(geometry);
                entry.snapshot = Some(snapshot);
                errors::clear();
                1
            }
            Err(code) => {
                errors::set(code);
                0
            }
        }
    })
}

/// The force stage registry index `layout_id` names, run over `entry`'s topology with
/// `replica` dividing its gathers, or why it did not run.
///
/// **Only the two threaded force stages are accepted**, and the test is the layout's *own*
/// registered id rather than a position in a list written here: a registry row that moves
/// must not silently become a different stage's replica run, and a stage that was never
/// split (grid, sugiyama, mds, …) has no `run_with`, so routing it here would have to
/// invent one. Everything else is `Code::UnknownLayoutId`, the code `gm_run` already uses
/// for an id this build cannot honour.
fn run_force(entry: &Handle, layout_id: u32, replica: &Replica) -> Result<Geometry, Code> {
    let Some(layout) = LAYOUTS.get(layout_id as usize) else {
        return Err(Code::UnknownLayoutId);
    };
    // The registry's `run` takes no parameters (C2), so the frozen defaults are the ones a
    // `gm_run` of the same layout would have used — same bytes, whichever path ran them.
    let params = ForceParams::default();
    let (topology, workers) = (&entry.topology, replica.ranks());
    if layout.id == BarnesHut::ID {
        BarnesHut::run_with(topology, &params, replica, workers).map_err(|_| Code::LayoutFailed)
    } else if layout.id == ParticleMesh::ID {
        ParticleMesh::run_with(topology, &params, replica, workers).map_err(|_| Code::LayoutFailed)
    } else {
        Err(Code::UnknownLayoutId)
    }
}

/// The run's two faces, kept together as `gm_run` keeps them: the layout's own geometry for
/// a POST pass, and the snapshot the column views read. Refused rather than half-set, so a
/// handle never carries one without the other.
fn paired(entry: &Handle, geometry: Geometry) -> Result<(Geometry, Snapshot), Code> {
    let snapshot = graph_core::layout::snapshot(&entry.topology, geometry.clone())
        .map_err(|_| Code::LayoutFailed)?;
    Ok((geometry, snapshot))
}
