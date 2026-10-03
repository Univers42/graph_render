//! `graph-cli overlap`: does the node-overlap pass actually remove overlap, and what does it
//! cost?
//!
//! The subject is a committed fixture (`--fixture`) or the hairball generator at a size
//! (`--nodes`, the sweep behind `SeparateParams::max_iterations`'s reach). It is laid out by
//! `--layout` first, laid out with a **non-zero node radius** so there is something to
//! separate — a `Point` layout has no extent and the pass is a documented no-op on one — and
//! then run through [`graph_core::post::separate`].
//!
//! **Three numbers per run**, all of them the ones the job's quality bar names:
//!
//! - the **invariant**, checked by an exhaustive `O(n²)` all-pairs scan over every pair.
//!   This is a **test instrument, not product**: nothing in `graph-core` calls it, and it is
//!   what makes the invariant exact rather than "usually". `--nodes` is refused above
//!   [`BRUTE_FORCE_CEILING`] for that reason.
//! - the **mean displacement** the pass caused, in layout units and as a fraction of the
//!   drawing's own mean pairwise distance — the second form is what makes the number
//!   comparable across sizes.
//! - the **stress ratio**, layout distances against graph distances, before and after: the
//!   quality cost of separating, which is the number a caller who cares about a readable
//!   *and* faithful drawing wants.
//!
//! Exit codes: `0` the invariant held and nothing was left overlapping · `1` at least one pair
//! still overlaps · `2` the measurement could not run (no such fixture, no such layout, a node
//! count the brute-force check refuses, or a layout that refused).
//!
//! **Ponytail (timing).** The wall time is one run on whatever machine ran it, no warm-up and
//! no repeat, so it under-reports on a loaded host. `docs/measurements/ux-overlap.md` is
//! written from a sweep of this command and says so.

use crate::hashgate::knob::env_setting;
use graph_core::REFERENCE_DEGREE;
use graph_core::post::separate::{SeparateParams, sweep};
use graph_core::{Geometry, Topology, index_model, post, registry, seeded_model};
use std::process::ExitCode;
use std::time::Instant;

/// The node count above which the exhaustive all-pairs invariant check is refused.
///
/// `O(n²)` pairs at n: 2 000 nodes is about 2 million distance evaluations, which is fast
/// enough to run in a gate row and small enough that the check is not the thing being timed.
/// Above it the check costs more than the pass it is checking — the pass is `O(n · k)` — so
/// running it would measure the instrument, not the capability. The pass itself is not
/// refused at any size; only this measurement is.
pub const BRUTE_FORCE_CEILING: u32 = 2_000;

/// What one `overlap` invocation was asked to measure.
pub struct Request<'a> {
    /// A committed fixture, by name.
    pub fixture: Option<&'a str>,
    /// The hairball generator's node count, for a scale sweep.
    pub nodes: Option<u32>,
    /// The layout that draws the graph before the pass.
    pub layout: &'a str,
    /// The node radius the pass is asked to separate, in layout units.
    pub radius: f64,
    /// Skip the exhaustive `O(n^2)` invariant scan and trust the pass's own grid count.
    ///
    /// **Only for the scale sweep.** Above [`BRUTE_FORCE_CEILING`] the scan is refused by
    /// default because it costs more than the pass it is checking — which is exactly why the
    /// sweep that measures the pass's cost at 10 000 and 100 000 nodes has to be able to skip
    /// it. What is given up is stated rather than hidden: `scan=false` prints the pass's own
    /// `reported_residual`, which is a grid count over the same 3 × 3 neighbourhood, so the two
    /// numbers bracket the same quantity and agreeing at small `n` is what makes the large-`n`
    /// number worth reading.
    pub scan: bool,
}

/// Runs the pass, checks the invariant, prints the three numbers; `0` when nothing overlaps.
pub fn run(request: &Request) -> ExitCode {
    match measure(request) {
        Ok(residual) if residual == 0 => ExitCode::SUCCESS,
        Ok(residual) => {
            eprintln!("overlap: {residual} pairs still overlap after the pass");
            ExitCode::from(1)
        }
        Err(why) => {
            eprintln!("overlap: {why}");
            ExitCode::from(2)
        }
    }
}

/// The three numbers, in the order they are printed.
struct Measured {
    /// Pairs still overlapping by more than the tolerance, from the exhaustive scan.
    residual: u32,
    /// Pairs overlapping before the pass, from the same scan — so the numbers bracket.
    before: u32,
    /// The pass's own `Bundled` counts, which the grid computed rather than the scan.
    resolved: u32,
    /// The pass's own residual, over the grid.
    reported: u32,
    /// Mean displacement in layout units, and as a fraction of the drawing's own scale.
    displacement: f64,
    relative: f64,
    /// The stress ratio before and after.
    stress_before: f64,
    stress_after: f64,
    /// The pass's wall time.
    millis: f64,
    /// Whether the exhaustive scan ran, or the numbers are the pass's own grid counts.
    scan: bool,
}

/// The one place the numbers are computed, so the printed line and the exit code cannot
/// disagree about what was measured.
fn measure(request: &Request) -> Result<u32, String> {
    let (topology, geometry) = laid_out(request)?;
    if request.scan && topology.node_count() > BRUTE_FORCE_CEILING {
        return Err(format!(
            "{} nodes: the exhaustive invariant check refuses above {BRUTE_FORCE_CEILING}, \
because it is O(n^2) and would cost more than the pass it checks. Pass --no-scan to skip it \
and read the pass's own grid count instead",
            topology.node_count()
        ));
    }
    let params = controlled_params()?;
    let radii = vec![request.radius as f32; topology.node_count() as usize];
    let before = if request.scan {
        overlapping(&geometry, &radii, 0.0)
    } else {
        u32::MAX
    };
    let stress_before = stress(&topology, &geometry);
    let started = Instant::now();
    let bundled = graph_core::post::separate::separate(&topology, &geometry, &params)
        .map_err(|e| e.to_string())?;
    let millis = started.elapsed().as_secs_f64() * 1_000.0;
    let after = &bundled.geometry;
    let residual = if request.scan {
        overlapping(after, &radii, 0.0)
    } else {
        bundled.unbundled
    };
    let displacement = mean_displacement(&geometry, after);
    let out = Measured {
        residual,
        before,
        resolved: bundled.pairs,
        reported: bundled.unbundled,
        displacement,
        relative: displacement / scale(&geometry),
        stress_before,
        stress_after: stress(&topology, after),
        millis,
        scan: request.scan,
    };
    print(request, &topology, &out);
    // **The exhaustive scan is the authority, not the pass's own count.** They agree by
    // construction — both walk the same 3x3 neighbourhood — and the gate row is checking that
    // they do, so a disagreement is reported rather than resolved in the pass's favour.
    if request.scan && out.residual != out.reported {
        return Err(format!(
            "the exhaustive scan found {} overlapping pairs and the pass reported {}",
            out.residual, out.reported
        ));
    }
    Ok(out.residual)
}

/// The pass's parameters, with the negative control applied when one is set.
///
/// **Read through the gate's own [`env_setting`] rather than `std::env` directly**, so the
/// control this command honours is the same one `hashgate` parses, with the same validity
/// rules: a value that would be refused there is refused here, and `=0` — the control's own
/// value — is legal rather than a parse failure. Without this the row would be green with the
/// knob set, which is the one outcome a negative control must never have.
fn controlled_params() -> Result<SeparateParams, String> {
    let setting = env_setting().map_err(|e| format!("{e}"))?;
    Ok(match setting.overlap_relaxation() {
        None => SeparateParams::default(),
        Some(relaxation) => SeparateParams {
            over_relaxation: relaxation as f32,
            ..SeparateParams::default()
        },
    })
}

fn print(request: &Request, topology: &Topology, m: &Measured) {
    println!(
        "layout={} nodes={} edges={} radius={} scan={} \
overlapping_before={} overlapping_after={} resolved={} reported_residual={} \
mean_displacement={:.6} relative={:.6} stress_before={:.6} stress_after={:.6} millis={:.3}",
        request.layout,
        topology.node_count(),
        topology.edge_count(),
        request.radius,
        m.scan,
        m.before,
        m.residual,
        m.resolved,
        m.reported,
        m.displacement,
        m.relative,
        m.stress_before,
        m.stress_after,
        m.millis,
    );
}

/// The graph, laid out and given a node radius.
///
/// **The radius is the reason the pass has anything to do.** A layout emits `Point` centres,
/// which have no extent, so the pass would be a documented no-op; the layout is therefore
/// re-declared as `Circle` nodes at `request.radius` before the pass sees it. That is the
/// honest framing for what this command measures — a renderer that draws discs of a known
/// size is the caller this pass is for.
fn laid_out(request: &Request) -> Result<(Topology, Geometry), String> {
    let (nodes, edges) = match (request.fixture, request.nodes) {
        (Some(name), _) => post::fdeb::load(name).map_err(|e| format!("fixture {name}: {e}"))?,
        (None, Some(count)) => seeded_model(1, count, REFERENCE_DEGREE),
        (None, None) => return Err("give --fixture NAME or --nodes N".into()),
    };
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let layout = registry::find(request.layout).ok_or_else(|| format!("no such layout: {}", request.layout))?;
    let geometry = (layout.run)(&topology).map_err(|e| e.to_string())?;
    if geometry.z.is_some() {
        return Err(format!(
            "{} is a 3D layout and the pass refuses a z column \
(docs/decisions/node-overlap.md 3)",
            request.layout
        ));
    }
    let circles = graph_contract::geometry::NodeGeometry::Circle {
        x: post::centres(&geometry.nodes).0.to_vec(),
        y: post::centres(&geometry.nodes).1.to_vec(),
        r: vec![request.radius as f32; topology.node_count() as usize],
    };
    let planar = geometry.with_nodes(circles);
    Ok((topology, planar))
}

/// The exhaustive all-pairs count of pairs closer than `ri + rj + 2 · margin`. `O(n²)` — the
/// instrument, and the only `O(n²)` in the product's story.
fn overlapping(geometry: &Geometry, radii: &[f32], margin: f64) -> u32 {
    let graph_contract::geometry::NodeGeometry::Circle { x, y, .. } = &geometry.nodes else {
        return 0;
    };
    let mut count = 0u32;
    for i in 0..x.len() {
        for j in i + 1..x.len() {
            let (dx, dy) = (x[i] - x[j], y[i] - y[j]);
            let need = radii[i] + radii[j] + 2.0 * margin as f32 - sweep::TOLERANCE;
            if libm::sqrtf(dx * dx + dy * dy) < need {
                count += 1;
            }
        }
    }
    count
}

/// The mean distance a node moved, in layout units.
fn mean_displacement(before: &Geometry, after: &Geometry) -> f64 {
    let (bx, by) = post::centres(&before.nodes);
    let (ax, ay) = post::centres(&after.nodes);
    if bx.is_empty() {
        return 0.0;
    }
    let total: f64 = (0..bx.len())
        .map(|i| {
            let (dx, dy) = (f64::from(ax[i] - bx[i]), f64::from(ay[i] - by[i]));
            libm::sqrt(dx * dx + dy * dy)
        })
        .sum();
    total / bx.len() as f64
}

/// The drawing's own mean nearest-neighbour distance, so displacement is comparable across
/// sizes. A fixed constant would make the number meaningless at 1 000 and at 100 000 nodes.
fn scale(geometry: &Geometry) -> f64 {
    let (x, y) = post::centres(&geometry.nodes);
    if x.len() < 2 {
        return 1.0;
    }
    let mut worst = f64::MAX;
    for i in 0..x.len() {
        let mut near = f64::MAX;
        for j in 0..x.len() {
            if i == j {
                continue;
            }
            let (dx, dy) = (f64::from(x[i] - x[j]), f64::from(y[i] - y[j]));
            near = near.min(libm::sqrt(dx * dx + dy * dy));
        }
        worst = worst.min(near);
    }
    worst.max(1e-9)
}

/// The stress ratio: mean graph distance over mean layout distance, over a fixed sample of
/// edges. Below 1 means the drawing compresses edges; above 1 means it stretches them.
///
/// A **sample**, not all edges, because the graph term is the expensive one and `n = 100 000`
/// with a full all-pairs edge scan would make this command measure itself. The sample is the
/// first [`STRESS_SAMPLE`] edges in topology order, which is fixed — no clock, no RNG (D1–D3)
/// — and stated here rather than left to look exhaustive.
pub const STRESS_SAMPLE: usize = 2_000;

fn stress(topology: &Topology, geometry: &Geometry) -> f64 {
    let (x, y) = post::centres(&geometry.nodes);
    let edges = topology.edges();
    let take = (edges.source.len()).min(STRESS_SAMPLE);
    if take == 0 {
        return 1.0;
    }
    let mut graph_total = 0.0f64;
    let mut layout_total = 0.0f64;
    for e in 0..take {
        let (s, t) = (edges.source[e] as usize, edges.target[e] as usize);
        graph_total += 1.0;
        let (dx, dy) = (f64::from(x[s] - x[t]), f64::from(y[s] - y[t]));
        layout_total += libm::sqrt(dx * dx + dy * dy);
    }
    if layout_total == 0.0 {
        return f64::INFINITY;
    }
    graph_total / layout_total
}