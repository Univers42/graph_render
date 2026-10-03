//! `layout.forceatlas2` against networkx 3.6's own `forceatlas2_layout`
//! (`harness/oracle-fa2.py`), started from the port's own initial positions so the two
//! arms run the same iterations from the same state.
//!
//! The port's own default is networkx's own 100 iterations, and at 100 this differential
//! cannot gate anything: ForceAtlas2 is chaotic, so networkx run against *itself* from a
//! start scaled by one ulp diverges *further* than the port does (max 2.5e-01 against the
//! port's 4.2e-02 at that budget, and 6x worse on the median seed — measured, not
//! estimated). The differential therefore gates at [`GATED_MAX_ITER`], the largest
//! iteration count at which networkx's own one-ulp self-divergence still stays under
//! 1e-6 across all 600 gate models, which is where a coordinate difference of the port's
//! own size is a real disagreement rather than the dynamics amplifying arithmetic. The
//! full-100-iteration comparison is reported in `docs/measurements/fa2-chaos.md` and gates
//! nothing: at that budget no coordinate tolerance is honest, in either direction.
//!
//! Ponytail (chaos): the gate's premise is arithmetic, not stability — the further the two
//! arms run, the less a coordinate gap means. Failing input: any graph dense enough that
//! one ulp of summation order grows over [`GATED_MAX_ITER`] iterations, which is what the
//! 100-iteration worst case is made of. Direction: the gap is over-stated, never
//! under-stated — a real port bug is caught at every budget (`GM_MUTATE_FA2_SCALING_RATIO`
//! measures 2.1e-01 against this 1e-7 ceiling, six decades over), and the price is a
//! false alarm if the port's shape changes. Escape hatch, from both ends: `--max-iter` on
//! `emit-fa2-fixtures` re-measures the whole comparison at any other budget without
//! touching the metric, and the same knob perturbs the port against the same reference,
//! so the ceiling is falsifiable from either side. The compared coordinates are ours
//! after the snapshot's f32 rounding, a floor near 1e-7 of the extent.

use super::{Differential, points};
use graph_core::layout::forceatlas2::{self, Fa2Params, ForceAtlas2, initial_positions};
use graph_core::{REFERENCE_DEGREE, Stage, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const FA2: Differential = Differential {
    name: "fa2",
    ceilings: &[
        ("layout.forceatlas2", "fa2", CEILING),
        // The 3D arm's own row, under its own key: it is one kernel at another `dim`, but
        // it is a different picture from a different start in a different space, and one
        // record standing for both would let either be measured by the other's run.
        ("layout.forceatlas2.3d", "fa2_3d", CEILING_3D),
    ],
    line,
};

/// The 3D arm's own ceiling: measured worst 2.985e-08 over the 1000 gate seeds at
/// `dim = 3`, rounded up to the next power of ten (`docs/measurements/p12-t4b.md`).
///
/// **It lands on the same 1e-7 as the 2D arm, and that is the measurement's finding, not an
/// inheritance.** A third coordinate is one more term in a difference vector, not another
/// source of divergence: at the same gated budget ([`GATED_MAX_ITER`], the largest at which
/// networkx's own one-ulp self-divergence stays under 1e-6) the 3D arm's worst coordinate
/// gap is 2.985e-08 against the 2D arm's 3.156e-08, and their medians agree to a tenth of a
/// decade. It is written as its own constant, with its own measured worst in the doc,
/// because "the 3D arm inherited the 2D number" would be a claim this differential cannot
/// support if the next run moved — and a future widening would then be invisible.
pub(crate) const CEILING_3D: f64 = 1e-7;

/// The iteration budget the coordinate differential gates at, against the library
/// default of 100 that `Fa2Params::default()` keeps: the largest budget at which
/// networkx's own one-ulp self-divergence stays under 1e-6 over every gate model
/// (`harness/fa2-chaos.py`; seeds 0..599 are the whole model set, since
/// `gate_node_count(seed) = 2 + seed % 600`). Budget 3 already measures 1.5e-6.
pub const GATED_MAX_ITER: u32 = 2;

/// The worst `max |ours - theirs| / extent(theirs)` at [`GATED_MAX_ITER`] over all 600
/// gate models (3.156e-08), rounded up to the next power of ten: a tolerance sitting
/// exactly on the number it was measured from is not one
/// (`docs/measurements/fa2-chaos.md`).
pub(crate) const CEILING: f64 = 1e-7;

fn line(seed: u32, max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    let reference = params(max_iter);
    let geometry =
        ForceAtlas2::run(&topology, &perturbed(reference)?).map_err(|e| e.to_string())?;
    let start = initial_positions(n, reference.seed, 2);
    let start_3d = initial_positions(n, reference.seed, 3);
    Ok(json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "params": {
            "max_iter": reference.max_iter, "jitter_tolerance": reference.jitter_tolerance,
            "scaling_ratio": reference.scaling_ratio, "gravity": reference.gravity,
            "dim": 2,
        },
        "initial": { "x": column(&start, 0), "y": column(&start, 1) },
        "fa2": points("layout.forceatlas2", &geometry.nodes)?,
        // The 3D arm's own start and its own reference call: networkx is handed a
        // three-column `pos`, which is how `forceatlas2_layout` is told the dimension
        // (`layout.py:1698-1700` — with `pos` given, `dim` is the width of `pos`).
        "params_3d": {
            "max_iter": reference.max_iter, "jitter_tolerance": reference.jitter_tolerance,
            "scaling_ratio": reference.scaling_ratio, "gravity": reference.gravity,
            "dim": 3,
        },
        "initial_3d": {
            "x": column(&start_3d, 0), "y": column(&start_3d, 1),
            "z": column(&start_3d, 2),
        },
        "fa2_3d": in_space_run(
            "layout.forceatlas2.3d",
            forceatlas2::run_3d(&topology, &reference),
        )?,
    }))
}

/// One coordinate of a start, as JSON. The 3D arm's `z` at `dim` 2 is absent rather than
/// a column of zeros, so the harness can tell "the reference ran in 2D" from "the
/// reference ran in 3D and every z happened to be zero".
fn column(start: &[[f64; 3]], axis: usize) -> Value {
    json!(start.iter().map(|p| p[axis]).collect::<Vec<_>>())
}

/// A 3D layout's three columns, or a refusal that names the id — the same
/// "only a point layout is a coordinate comparison" rule [`points`] states for 2D.
fn in_space(id: &str, geometry: &graph_core::layout::Geometry) -> Result<Value, String> {
    let mut out = points(id, &geometry.nodes)?;
    match &geometry.z {
        Some(z) => {
            out["z"] = json!(z);
            Ok(out)
        }
        // A 3D id that answered without a z column would be a 2D picture wearing a 3D
        // name, and the harness would compare the wrong thing.
        None => Err(format!("{id}: expected a z column, got none")),
    }
}

/// [`in_space`] over a run that has not happened yet, with the refusal kept readable.
fn in_space_run(
    id: &str,
    run: Result<graph_core::layout::Geometry, impl std::fmt::Display>,
) -> Result<Value, String> {
    in_space(id, &run.map_err(|e| e.to_string())?)
}

/// What networkx is to be run with: the gated budget, or `--max-iter`'s override. Never
/// the perturbation — the fixture describes the *reference*, so a perturbed port is
/// measured against the unperturbed algorithm and the gap is what it should be.
fn params(max_iter: Option<u32>) -> Fa2Params {
    Fa2Params {
        max_iter: max_iter.unwrap_or(GATED_MAX_ITER),
        dim: 2,
        ..Fa2Params::default()
    }
}

/// `params` with `GM_MUTATE_FA2_SCALING_RATIO` applied, so the differential's own
/// negative control measures a perturbed port against the same reference
/// (`hashgate::fa2_perturbation`, which refuses a typo'd or doubled knob rather than
/// falling back to the default and passing as green).
fn perturbed(params: Fa2Params) -> Result<Fa2Params, String> {
    Ok(Fa2Params {
        max_iter: params.max_iter,
        ..crate::hashgate::fa2_perturbation()?
    })
}
