//! Running one motor layout the way SciGraphs would run its reference: the registered
//! default for almost every id, and a deliberate override for the six where the registered
//! default is not SciGraphs' parameter or not SciGraphs' units.
//!
//! **One convention, and it is applied to both arms rather than to the motor.** A Graphviz
//! row's reference is the engine's own `-Tplain` points, and SciGraphs never returns those:
//! it centres them on their mean, divides by their largest extent and multiplies by `scale`
//! (`yifan_hu.py:318-325`), a layer `scigraphs_utils.graphviz_layout` does inside the C++
//! extension. [`scigraphs_graphviz_post`] is that layer, and `harness/scigraphs-conformance/
//! sc_graphviz.py` applies the same five lines in Python to the reference arm. What is left
//! after it is the layout, not the unit.
//!
//! **Six overrides, and each is a whole row.** `CIRCLE_PACKING`'s registered budget is 500
//! radius-solver sweeps where `apply_graph_layout` passes 50; `FORCEATLAS2`'s is 100 where
//! the dispatcher passes 50 into `ForceSim`; `GRAPHVIZ_SFDP` registers `run`, whose
//! `DEFAULT_SEED` is 1, where the engine is handed `start = get_layout_seed()`;
//! `layout.dag.sugiyama` draws in the priority method's own units and `layer *
//! LAYER_SPACING`, where the reference maps each axis onto `[-scale, scale]`; `GRID`
//! registers a lattice centred on the origin at unit pitch, where `_grid_layout` starts at
//! the origin and pitches it at `scale / grid_size`; and `layout.random` registers
//! networkx's planar unit-square scatter off the crate's `Mulberry32`, where SciGraphs draws
//! `rand(n, 3) * scale` off MT19937 at the layout seed (`basic.py:5-9`). Every other id
//! either takes no parameter or its registered default already **is** the reference's —
//! the igraph family being the surprising half: `_igraph_davidson_harel` ignores the
//! dispatcher's `iterations` and uses igraph's `maxiter=10`, which is our `DhParams` default
//! too (`igraph_layouts.py:117-118`, `davidson_harel.rs:44`).
//!
//! Apart from that one layout's axes and the Graphviz convention both arms share, nothing
//! here normalises a coordinate. What the layout returns is what goes into the `.f64` file,
//! and every parameter the motor could not be given is a `Gap` in [`super::rows`], not a
//! number fudged to match.

mod overrides;

use super::SCALE;
use super::fixtures::Fixture;
use graph_contract::binary::SnapshotParts;
use graph_contract::geometry::NodeGeometry;
use graph_core::layout::Geometry;
use graph_core::layout::force::spring::{Spring, Spring3D};
use graph_core::{StageError, registry, run_with};
use overrides::{fa2, grid, packing, random_seeded, sfdp_seeded, spring, sugiyama_scaled};
use serde_json::Value;

mod gv_post;
pub use gv_post::{GRAPHVIZ_DIMS, scigraphs_graphviz_post};

/// The environment variable that breaks one row's coordinates by one `f32` ULP, for the
/// `negctl-scigraphs-conformance` row.
///
/// **Bit 29, not bit 28.** A double's mantissa is 52 bits and a single's is 23, so the
/// lowest bit a single keeps is bit 29 of the double (52 - 23); bit 28 is the rounding bit
/// below it, and a flip there dies in the narrowing. One ULP of `f32`, on the other hand,
/// moves the single by a whole step and so moves the narrowed value — which is what makes
/// the break visible in both files the judge can read.
pub const BREAK_ENV: &str = "GM_MUTATE_SCIGRAPHS_CONFORMANCE";

/// The bit [`flip_one_bit`] flips, as a mask within byte 3 of the double's little-endian
/// bytes. Exposed so the test names the number rather than restating it.
pub const BREAK_MASK: u8 = 0b0010_0000;

/// The SciGraphs name whose coordinates [`break_one`] perturbs, if the run asked for one.
pub fn break_one() -> Option<String> {
    std::env::var(BREAK_ENV)
        .ok()
        .filter(|name| !name.is_empty())
}

/// One layout's coordinates for one fixture, or why it has none.
pub type Ran = Result<Vec<[f64; 3]>, String>;

/// Run `id` over `fixture` with SciGraphs' parameters, through the same `run_with` the
/// pipeline uses — so the coordinates compared are the snapshot's, not the geometry's.
pub fn run(id: &str, fixture: &Fixture) -> Ran {
    let parts = match id {
        "layout.packing.circle" => packing(fixture),
        "layout.forceatlas2" => fa2(fixture),
        "layout.force.sfdp" => sfdp_seeded(fixture),
        "layout.force.spring" => spring::<Spring>(fixture),
        "layout.force.spring3d" => spring::<Spring3D>(fixture),
        "layout.dag.sugiyama" => sugiyama_scaled(fixture),
        "layout.random" => random_seeded(fixture),
        "layout.grid" => grid(fixture),
        _ => registered(id, fixture),
    }?;
    columns(&parts, fixture.nodes.len())
}

/// [`run`], then the one convention a Graphviz row carries: SciGraphs' centre-and-rescale
/// over the engine's points (`yifan_hu.py:318-325`). A `Reference::Scigraphs` row already had
/// it applied inside `apply_graph_layout`, so applying it twice would be the bug, and a
/// `Reference::Graphviz` row is the only one whose reference arm takes the engine's raw
/// points.
pub fn run_row(row: &super::Row, fixture: &Fixture) -> Ran {
    let id = row.motor.ok_or("no motor layout")?;
    let points = run(id, fixture)?;
    if row.engine().is_some() {
        Ok(scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE))
    } else {
        Ok(points)
    }
}

/// Every other id at its registered default.
fn registered(id: &str, fixture: &Fixture) -> Result<SnapshotParts, String> {
    let layout = registry::find(id).ok_or_else(|| format!("{id}: not registered"))?;
    finish(fixture, layout.id, |t| (layout.run)(t))
}

/// One fixture through `run_with` with `id` named, so the snapshot is labelled as the layout
/// that produced it — and indexed once, by the pipeline, rather than twice.
fn finish(
    fixture: &Fixture,
    id: &'static str,
    layout: impl FnOnce(&graph_core::Topology) -> Result<Geometry, StageError>,
) -> Result<SnapshotParts, String> {
    run_with(&fixture.nodes, &fixture.edges, id, layout)
        .map(|run| run.snapshot.into_parts())
        .map_err(|e| e.to_string())
}

/// The three columns: `x` and `y` from whichever geometry kind the layout emitted, and `z`
/// from the snapshot's z column, `0.0` where the layout is 2D.
///
/// **A 2D layout's z is 0.0, and that is stated rather than filled in.** SciGraphs always
/// writes three coordinates (`_check_positions`, `dispatcher.py:149`) even for a planar
/// layout, so the third column is a real difference between the arms on most rows; reporting
/// it keeps a planar-against-planar row from reading as a 2D comparison.
fn columns(parts: &SnapshotParts, count: usize) -> Ran {
    let (x, y) = match &parts.nodes {
        NodeGeometry::Point { x, y, .. } => (x, y),
        NodeGeometry::Circle { x, y, .. } => (x, y),
        NodeGeometry::Box { x, y, .. } => (x, y),
    };
    if x.len() != count || y.len() != count {
        return Err(format!(
            "{} columns against {count} nodes",
            x.len().max(y.len())
        ));
    }
    let z = parts.z.as_deref();
    if let Some(column) = z
        && column.len() != count
    {
        return Err(format!("{} z values against {count} nodes", column.len()));
    }
    Ok((0..count)
        .map(|i| [x[i] as f64, y[i] as f64, z.map_or(0.0, |c| c[i] as f64)])
        .collect())
}

/// Flip bit 28 of the first `f64` in `bytes`, in place, and say so.
///
/// One double, one bit, no scaling: the negative control has to be the smallest thing that
/// can be wrong, or the row it guards is not the row it claims to guard.
pub fn flip_one_bit(bytes: &mut [u8]) -> Result<(), String> {
    let word = bytes.get_mut(..8).ok_or("no coordinate to break")?;
    word[3] ^= BREAK_MASK;
    Ok(())
}

/// The raw little-endian `f64` file for one row's coordinates, in fixture order.
///
/// **Little-endian and `f64` because that is what the reference arm reads.** The bytes are
/// written straight out of the layout with no JSON in the path, so a coordinate cannot be
/// rounded by a decimal round trip on its way to the comparison.
pub fn raw_f64(points: &[Ran], broken: bool) -> (Vec<u8>, u64) {
    let mut bytes = Vec::new();
    let mut count = 0;
    for run in points.iter().flatten() {
        for [x, y, z] in run {
            bytes.extend_from_slice(&x.to_le_bytes());
            bytes.extend_from_slice(&y.to_le_bytes());
            bytes.extend_from_slice(&z.to_le_bytes());
            count += 3;
        }
    }
    if broken && flip_one_bit(&mut bytes).is_err() {
        eprintln!("{BREAK_ENV}: no coordinate to break");
    }
    (bytes, count)
}

/// The `f32` file, derived from the same `f64` bytes so the two files cannot disagree about
/// a row `--break` perturbed. Little-endian in both widths.
pub fn raw_f32(f64_bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(f64_bytes.len() / 2);
    for word in f64_bytes.as_chunks::<8>().0 {
        let value = f64::from_le_bytes(*word) as f32;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

/// The one line `motor.jsonl` carries for a row: what ran, how many coordinates it produced,
/// which fixtures it did not run on, and every parameter the motor had no slot for.
pub fn row_line(
    row: &super::Row,
    points: &[Ran],
    names: &[&str],
    sha256: Option<String>,
    coordinates: u64,
    broken: bool,
) -> Value {
    let skipped: Vec<&str> = points
        .iter()
        .zip(names)
        .filter(|(run, _)| run.is_err())
        .map(|(_, name)| *name)
        .collect();
    let gaps: Vec<Value> = row
        .gaps
        .iter()
        .map(|gap| {
            serde_json::json!({
                "parameter": gap.parameter, "note": gap.note, "at": gap.at,
            })
        })
        .collect();
    let detail = match row.motor {
        None => "not run: no motor layout for this name".to_string(),
        Some(_) if points.iter().all(Result::is_err) => {
            "not run: the layout failed on every fixture".to_string()
        }
        Some(_) if !skipped.is_empty() => {
            format!("partial: no coordinates on {}", skipped.join(", "))
        }
        Some(_) if broken => format!("broken: {BREAK_ENV} flipped one bit"),
        Some(_) => "ok".to_string(),
    };
    serde_json::json!({
        "name": row.name,
        "motor_id": row.motor,
        "reference": match row.reference {
            super::Reference::Scigraphs => "scigraphs".to_string(),
            super::Reference::Graphviz(engine) => format!("graphviz:{engine}"),
        },
        "motor": detail,
        "fixtures": names.len(),
        "skipped": skipped,
        "coordinates": coordinates,
        "sha256": sha256,
        "broken": broken,
        "convention_gaps": gaps,
    })
}

#[cfg(test)]
mod tests;
