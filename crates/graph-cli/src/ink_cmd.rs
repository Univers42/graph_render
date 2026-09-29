//! `graph-cli ink`: how much ink a bundler saves on a graph, and what one pass costs.
//!
//! The subject is a committed POST fixture (`--fixture`) or the hairball generator at a size
//! (`--nodes`, the sweep behind a bundler's `scale_ceiling`). It is laid out by `--layout`
//! first — a bundler composes with every layout, so which one is a choice, and the default is
//! the circular layout because nodes on rings make the chords a bundler is for — then
//! measured straight and bundled by every registered capability.
//!
//! Exit codes: `0` every registered bundler reduced the occupied cells · `1` at least one did
//! not · `2` the measurement could not run (no such fixture, no such layout, an index or a
//! layout that refused).
//!
//! **Ponytail (timing).** The wall time is one run on whatever machine ran it, with no
//! warm-up and no repeat, so it under-reports on a loaded host and drifts by tens of percent
//! between runs. The ceiling in each capability's ledger row is set with that margin in hand,
//! never at the last size that squeaked in. Re-run the sweep to refresh it.

use graph_core::post;
use graph_core::post::ink::INK_RESOLUTION;
use graph_core::{Geometry, Topology, index_model, registry};
use std::process::ExitCode;
use std::time::Instant;

/// What one `ink` invocation was asked to measure.
pub struct Request<'a> {
    /// A committed POST fixture, by name.
    pub fixture: Option<&'a str>,
    /// The hairball generator's node count, for a scale sweep.
    pub nodes: Option<u32>,
    /// The layout that draws the graph before bundling.
    pub layout: &'a str,
}

/// Measures and prints; `0` when every registered bundler reduced the occupied cells.
pub fn run(request: &Request) -> ExitCode {
    match measure(request) {
        Ok(reduced) if reduced => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(why) => {
            eprintln!("ink: {why}");
            ExitCode::from(2)
        }
    }
}

fn measure(request: &Request) -> Result<bool, String> {
    let (topology, geometry) = laid_out(request)?;
    let before = post::measure(&topology, &geometry);
    println!(
        "straight  layout={} nodes={} edges={} cells={} length={:.3} resolution={INK_RESOLUTION}",
        request.layout,
        topology.node_count(),
        topology.edge_count(),
        before.cells,
        before.length
    );
    let mut all_reduced = true;
    for capability in &post::POSTS {
        let started = Instant::now();
        let bundled = (capability.run)(&topology, &geometry)
            .map_err(|e| format!("{}: {e}", capability.id))?;
        let millis = started.elapsed().as_secs_f64() * 1000.0;
        let after = post::measure(&topology, &bundled.geometry);
        println!(
            "{}  cells={} length={:.3} ink_reduction={:.2}% pairs={} unbundled={} millis={millis:.1}",
            capability.id,
            after.cells,
            after.length,
            reduction(before.cells, after.cells),
            bundled.pairs,
            bundled.unbundled
        );
        all_reduced &= after.cells < before.cells;
    }
    Ok(all_reduced)
}

/// The ink reduction in percent, to two figures. The straight line is the denominator, and it
/// is the raster's cell count rather than a length: bundling cannot shorten a path, so a
/// length ratio would read as a regression no matter how tight the bundle got.
fn reduction(before: u32, after: u32) -> f64 {
    if before == 0 {
        return 0.0;
    }
    100.0 * (1.0 - f64::from(after) / f64::from(before))
}

fn laid_out(request: &Request) -> Result<(Topology, Geometry), String> {
    let (nodes, edges) = match (request.fixture, request.nodes) {
        (Some(name), None) => post::fdeb::load(name)?,
        (None, Some(count)) => post::fdeb::hairball(count),
        _ => return Err("pass exactly one of --fixture or --nodes".into()),
    };
    let id = request
        .layout
        .strip_prefix("layout.")
        .unwrap_or(request.layout);
    let layout = registry::find(&format!("layout.{id}")).ok_or_else(|| {
        format!(
            "no layout {}; the registered ones are {}",
            request.layout,
            layout_names()
        )
    })?;
    let topology = index_model(&nodes, &edges).map_err(|e| format!("index: {e:?}"))?;
    let geometry = (layout.run)(&topology).map_err(|e| format!("{}: {e}", layout.id))?;
    Ok((topology, geometry))
}

/// Every registered layout id, for the refusal message: an unknown name is a typo far more
/// often than it is a missing layout, so the message lists what there is.
fn layout_names() -> String {
    let mut names: Vec<&str> = registry::LAYOUTS.iter().map(|layout| layout.id).collect();
    names.sort_unstable();
    names.join(", ")
}
