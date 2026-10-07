//! `graph-cli gpu-stress`: the GPU tier's layout quality — the GPU tick's own f32
//! positions against the CPU mesh's stress-1 over the same graph, from the same start,
//! for the same tick count.
//!
//! The GPU tier's resident tick writes final f32 positions to `target/gpu-tick/<fixture>.f32`
//! (raw Float32Array, x,y interleaved, 2n components). This command reads those positions,
//! rebuilds the CPU mesh from the fixture's edges, steps it the same number of ticks from
//! the same start positions, and compares the two layouts' Kruskal stress-1
//! ([`crate::bench::stress`]).
//!
//! The ratio must be in [0.5, 2.0]: the GPU's layout quality is within a factor of two of
//! the CPU mesh's. At `--ticks 1` the positions themselves are held too: the one-tick
//! displacement's rmsRel must stay under [`DISPLACEMENT_CEILING`], which is the check the
//! tick's two faults fail. Exit 0 on pass, 1 on fail, 2 if it could not run.
//!
//! Caveat: stress is quadratic — BFS from SOURCES sources over the whole graph — so this
//! subcommand is only run at 10k and 50k, where the BFS is affordable. No guard is added;
//! the caller chooses the size.

use crate::bench::stress;
use crate::command::GpuStressPlan;

/// The plan `command::Command::GpuStress` carries, under the path the other variants' plans
/// resolve at (`crate::bench::tick::Plan`, `crate::mb_fidelity::Plan`).
pub use crate::command::GpuStressPlan as Plan;
use graph_core::layout::force::{ForceParams, ForceSession, LiveParams};
use graph_core::{EdgeKind, NodeKind, Topology, index_model};
use std::process::ExitCode;

/// `graph-cli gpu-stress <fixture.gmfx> <positions.f32> --ticks T`.
pub fn run(plan: &GpuStressPlan) -> ExitCode {
    match compare(plan) {
        Ok((ratio, displacement)) => {
            let pass = passes(plan.ticks, ratio, displacement);
            println!(
                "{} ratio={:.6} displacementRelRms={:.3e}",
                if pass { "PASS" } else { "FAIL" },
                ratio,
                displacement
            );
            if pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            eprintln!("gpu-stress: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// The stress ratio's pass band: the GPU's stress may be at most twice or at least half
/// the CPU mesh's.
const PASS_MIN: f64 = 0.5;
const PASS_MAX: f64 = 2.0;

/// The one-tick displacement's ceiling: the decade above the largest measured, `1.6e-5` on
/// `mesh-50k-settled`, and under the plan's `1e-3` guard. The weakest fault measured `0.18`
/// (`docs/measurements/gpu-g1.md`, G1c).
pub(crate) const DISPLACEMENT_CEILING: f64 = 1e-4;

/// The verdict: the ratio in its band, and at one tick the displacement under its ceiling.
/// Over many ticks the two layouts diverge chaotically, so only the ratio is held there.
fn passes(ticks: u32, ratio: f64, displacement: f64) -> bool {
    (PASS_MIN..=PASS_MAX).contains(&ratio) && (ticks != 1 || displacement <= DISPLACEMENT_CEILING)
}

/// Reads both files, rebuilds the CPU mesh, and returns the GPU/CPU stress ratio and the
/// one-tick displacement's rmsRel.
fn compare(plan: &GpuStressPlan) -> Result<(f64, f64), String> {
    let fixture = Fixture::read(&plan.fixture)?;
    let gpu = Positions::read(&plan.positions, fixture.n)?;
    ratio_over(&fixture, &gpu, plan.ticks)
}

/// The GPU/CPU stress ratio and the one-tick displacement's rmsRel: rebuild the CPU mesh
/// from the fixture's edges, step it `ticks` ticks from the fixture's start positions, and
/// compare both layouts' stress-1 over the same graph.
///
/// The displacement is `rms(gpu − cpu) / rms(cpu − start)` over the `2n` axis components:
/// how far the GPU's one-tick positions are from the CPU's, relative to how far the CPU's
/// are from the start. It is the check the tick's controls fail.
fn ratio_over(fixture: &Fixture, gpu: &Positions, ticks: u32) -> Result<(f64, f64), String> {
    let topology = topology_from_edges(fixture)?;
    let mut session = ForceSession::from_positions(
        &topology,
        LiveParams::from(ForceParams::default()),
        &fixture.start_x,
        &fixture.start_y,
    )
    .map_err(|e| e.to_string())?
    .with_particle_mesh();
    session.step(ticks);
    let cpu_x: Vec<f32> = session.xs().iter().map(|&v| v as f32).collect();
    let cpu_y: Vec<f32> = session.ys().iter().map(|&v| v as f32).collect();
    let gpu_stress = stress(&gpu.x, &gpu.y, &fixture.lo, &fixture.hi);
    let cpu_stress = stress(&cpu_x, &cpu_y, &fixture.lo, &fixture.hi);
    if cpu_stress == 0.0 {
        return Err("the CPU mesh's stress is zero: every pair sits at its hop distance".into());
    }
    let displacement = displacement_rel_rms(gpu, &cpu_x, &cpu_y, fixture);
    Ok((gpu_stress / cpu_stress, displacement))
}

/// `rms(gpu − cpu) / rms(cpu − start)` over the `2n` axis components, in `f64`.
///
/// The reference is the CPU's own displacement from the start, so the number is relative to
/// how far the CPU mesh moved in one tick — an absolute bound would be a statement about the
/// layout's scale and not about the tick's fidelity.
fn displacement_rel_rms(gpu: &Positions, cpu_x: &[f32], cpu_y: &[f32], fixture: &Fixture) -> f64 {
    let n = fixture.n as usize;
    let mut sum_diff = 0.0_f64;
    let mut sum_ref = 0.0_f64;
    for i in 0..n {
        let dx = f64::from(gpu.x[i] - cpu_x[i]);
        let dy = f64::from(gpu.y[i] - cpu_y[i]);
        sum_diff += dx * dx + dy * dy;
        let rx = f64::from(cpu_x[i] - fixture.start_x[i] as f32);
        let ry = f64::from(cpu_y[i] - fixture.start_y[i] as f32);
        sum_ref += rx * rx + ry * ry;
    }
    let count = (2 * n) as f64;
    let rms_diff = (sum_diff / count).sqrt();
    let rms_ref = (sum_ref / count).sqrt();
    if rms_ref == 0.0 {
        return 0.0;
    }
    rms_diff / rms_ref
}

/// Builds a [`Topology`] from the fixture's edges: `n` nodes with synthetic ids, and one
/// edge per `(lo, hi)` pair referencing them, so `index_model` resolves the endpoints to
/// the same dense indices the fixture carries.
fn topology_from_edges(fixture: &Fixture) -> Result<Topology, String> {
    let mut nodes = Vec::with_capacity(fixture.n as usize);
    for i in 0..fixture.n {
        nodes.push(graph_core::NodeRecord {
            id: format!("n{i}"),
            kind: NodeKind::Record,
            database_id: None,
            source: "gpu".into(),
            label: String::new(),
            group: None,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon: None,
        });
    }
    let mut edges = Vec::with_capacity(fixture.lo.len());
    let ends = fixture.lo.iter().zip(&fixture.hi);
    for (e, ((&s, &t), &strength)) in ends.zip(&fixture.strength).enumerate() {
        edges.push(graph_core::EdgeRecord {
            id: format!("e{e}"),
            source: format!("n{s}"),
            target: format!("n{t}"),
            kind: EdgeKind::Relation,
            label: String::new(),
            strength,
            directed: false,
            record_id: None,
            child_first: false,
        });
    }
    index_model(&nodes, &edges).map_err(|e| format!("building the topology: {e}"))
}

mod reader;
use reader::{Fixture, Positions};

#[cfg(test)]
#[path = "gpu_stress/tests.rs"]
mod tests;
