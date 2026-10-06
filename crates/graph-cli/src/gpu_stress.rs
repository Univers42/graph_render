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
//! the CPU mesh's. Exit 0 on pass, 1 on fail, 2 if it could not run.
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
use std::path::Path;
use std::process::ExitCode;

/// The `.gmfx` magic, the same four bytes `gpu_fixtures::emit` writes.
const MAGIC: [u8; 4] = *b"GMFX";

/// The `format major` this reader knows, the one `gpu_fixtures::emit` writes.
const MAJOR: u32 = 1;

/// Header length in bytes: ten `u32` words, then three `f64`s.
const HEADER_LEN: usize = 64;

/// `graph-cli gpu-stress <fixture.gmfx> <positions.f32> --ticks T`.
pub fn run(plan: &GpuStressPlan) -> ExitCode {
    match compare(plan) {
        Ok((ratio, displacement)) => {
            let pass = (PASS_MIN..=PASS_MAX).contains(&ratio);
            println!(
                "{} ratio={:.6} displacementRelRms={:.6}",
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

/// Reads both files, rebuilds the CPU mesh, and returns the GPU/CPU stress ratio and the
/// one-tick displacement's rmsRel.
fn compare(plan: &GpuStressPlan) -> Result<(f64, f64), String> {
    let fixture = Fixture::read(&plan.fixture)?;
    let gpu = Positions::read(&plan.positions, fixture.n)?;
    ratio_over(&fixture, &gpu, plan.ticks)
}

/// The graph and the start positions one `.gmfx` carries: the sections this command reads,
/// and nothing the mesh's own solve would fill in.
struct Fixture {
    /// Node count `n`.
    n: u32,
    /// Each simple edge's lower-index endpoint, in edge order.
    lo: Vec<u32>,
    /// Each simple edge's higher-index endpoint, in the same order.
    hi: Vec<u32>,
    /// The session's own start `x` column.
    start_x: Vec<f64>,
    /// The session's own start `y` column.
    start_y: Vec<f64>,
}

impl Fixture {
    /// Reads and parses one `.gmfx`, refusing a wrong magic, an unknown major, or a
    /// truncated payload.
    fn read(path: &Path) -> Result<Fixture, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let n = header(&bytes, path)?;
        let m = u32_at(&bytes, 16);
        let need = HEADER_LEN + 8 + 8 * m as usize + 16 * n as usize;
        if bytes.len() < need {
            return Err(format!(
                "{}: the file holds {need} bytes of header, edges and positions, and the file is {} long",
                path.display(),
                bytes.len()
            ));
        }
        let mut at = HEADER_LEN + 8;
        let lo = u32s(&bytes, at, m, path)?;
        at += 4 * m as usize;
        let hi = u32s(&bytes, at, m, path)?;
        at += 4 * m as usize;
        at += 8 * m as usize;
        let start_x = f64s(&bytes, at, n, path)?;
        at += 8 * n as usize;
        let start_y = f64s(&bytes, at, n, path)?;
        Ok(Fixture {
            n,
            lo,
            hi,
            start_x,
            start_y,
        })
    }
}

/// The GPU tick's final positions: raw f32, x,y interleaved, 2n components.
struct Positions {
    /// Per-node `x`, narrowed to f32.
    x: Vec<f32>,
    /// Per-node `y`, narrowed to f32.
    y: Vec<f32>,
}

impl Positions {
    /// Reads `2n` f32 words as `n` interleaved `x,y` pairs, refusing a length that does
    /// not follow from the fixture's own `n`.
    fn read(path: &Path, n: u32) -> Result<Positions, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
        let want = 8 * n as usize;
        if bytes.len() != want {
            return Err(format!(
                "{}: the file holds {} bytes, and {n} nodes need {want}",
                path.display(),
                bytes.len()
            ));
        }
        let mut x = Vec::with_capacity(n as usize);
        let mut y = Vec::with_capacity(n as usize);
        for i in 0..n as usize {
            x.push(f32_at(&bytes, 8 * i));
            y.push(f32_at(&bytes, 8 * i + 4));
        }
        Ok(Positions { x, y })
    }
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
    for (e, (&s, &t)) in fixture.lo.iter().zip(&fixture.hi).enumerate() {
        edges.push(graph_core::EdgeRecord {
            id: format!("e{e}"),
            source: format!("n{s}"),
            target: format!("n{t}"),
            kind: EdgeKind::Relation,
            label: String::new(),
            strength: 1.0,
            directed: false,
            record_id: None,
            child_first: false,
        });
    }
    index_model(&nodes, &edges).map_err(|e| format!("building the topology: {e}"))
}

/// The header's own checks — magic, major — and the node count they carry.
fn header(bytes: &[u8], path: &Path) -> Result<u32, String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!(
            "{}: the file is {} bytes, shorter than the {HEADER_LEN}-byte header",
            path.display(),
            bytes.len()
        ));
    }
    if bytes[0..4] != MAGIC {
        return Err(format!(
            "{}: the magic is not GMFX, so this is not a fixture",
            path.display()
        ));
    }
    let major = u32_at(bytes, 4);
    if major != MAJOR {
        return Err(format!(
            "{}: format major {major}, and this reader knows {MAJOR}",
            path.display()
        ));
    }
    Ok(u32_at(bytes, 12))
}

fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn f32_at(bytes: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(bytes[at..at + 4].try_into().expect("four bytes"))
}

fn f64_at(bytes: &[u8], at: usize) -> f64 {
    f64::from_le_bytes(bytes[at..at + 8].try_into().expect("eight bytes"))
}

/// `count` `u32` words at `at`, read in wire order.
fn u32s(bytes: &[u8], at: usize, count: u32, path: &Path) -> Result<Vec<u32>, String> {
    let end = at + 4 * count as usize;
    if end > bytes.len() {
        return Err(format!(
            "{}: a u32 section ends at {end} and the file holds {} bytes",
            path.display(),
            bytes.len()
        ));
    }
    Ok((0..count).map(|i| u32_at(bytes, at + 4 * i as usize)).collect())
}

/// `count` `f64` words at `at`, read in wire order.
fn f64s(bytes: &[u8], at: usize, count: u32, path: &Path) -> Result<Vec<f64>, String> {
    let end = at + 8 * count as usize;
    if end > bytes.len() {
        return Err(format!(
            "{}: an f64 section ends at {end} and the file holds {} bytes",
            path.display(),
            bytes.len()
        ));
    }
    Ok((0..count).map(|i| f64_at(bytes, at + 8 * i as usize)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::layout::force::{ForceParams, ForceSession, LiveParams};

    /// The CPU mesh's own positions, compared against themselves, give a ratio of exactly
    /// 1.0 — the plumbing (topology build, session step, file I/O, stress, ratio) is
    /// exercised end to end.
    #[test]
    fn a_gpu_stress_of_the_cpu_s_own_positions_is_one() {
        let n = 8u32;
        let lo: Vec<u32> = (0..n - 1).collect();
        let hi: Vec<u32> = (1..n).collect();
        let start_x: Vec<f64> = (0..n).map(|i| f64::from(i) * 10.0).collect();
        let start_y: Vec<f64> = (0..n).map(|i| f64::from(i % 3) * 5.0).collect();
        let fixture = Fixture {
            n,
            lo: lo.clone(),
            hi: hi.clone(),
            start_x: start_x.clone(),
            start_y: start_y.clone(),
        };
        let cpu = cpu_positions(&fixture, 3);
        let dir = std::env::temp_dir().join(format!("gm-gpu-stress-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let positions_path = dir.join("cpu.f32");
        let fixture_path = dir.join("mesh.gmfx");
        write_f32(&positions_path, &cpu.x, &cpu.y).unwrap();
        std::fs::write(&fixture_path, minimal_gmfx(&fixture)).unwrap();
        let plan = GpuStressPlan {
            fixture: fixture_path,
            positions: positions_path,
            ticks: 3,
        };
        let (ratio, _) = compare(&plan).unwrap();
        std::fs::remove_dir_all(&dir).ok();
        assert!((ratio - 1.0).abs() < 1e-6, "the CPU compared against itself is {ratio}");
    }

    /// The CPU mesh's final positions after `ticks` ticks from the fixture's start.
    fn cpu_positions(fixture: &Fixture, ticks: u32) -> Positions {
        let topology = topology_from_edges(fixture).unwrap();
        let mut session = ForceSession::from_positions(
            &topology,
            LiveParams::from(ForceParams::default()),
            &fixture.start_x,
            &fixture.start_y,
        )
        .unwrap()
        .with_particle_mesh();
        session.step(ticks);
        Positions {
            x: session.xs().iter().map(|&v| v as f32).collect(),
            y: session.ys().iter().map(|&v| v as f32).collect(),
        }
    }

    /// Writes raw f32 x,y interleaved, 2n components.
    fn write_f32(path: &Path, x: &[f32], y: &[f32]) -> Result<(), String> {
        let mut bytes = Vec::with_capacity(8 * x.len());
        for (&px, &py) in x.iter().zip(y) {
            bytes.extend_from_slice(&px.to_le_bytes());
            bytes.extend_from_slice(&py.to_le_bytes());
        }
        std::fs::write(path, &bytes).map_err(|e| format!("writing {}: {e}", path.display()))
    }

    /// A minimal `.gmfx`: the header, the edges and the start positions, with the mesh
    /// columns (twiddles, spectrum, deltas) as zeros — the parser reads only the first
    /// three sections.
    fn minimal_gmfx(fixture: &Fixture) -> Vec<u8> {
        let m = fixture.lo.len() as u32;
        let side = 128u32;
        let mut out = Vec::new();
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&MAJOR.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&fixture.n.to_le_bytes());
        out.extend_from_slice(&m.to_le_bytes());
        out.extend_from_slice(&side.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&1.0f64.to_le_bytes());
        out.extend_from_slice(&0.0f64.to_le_bytes());
        out.extend_from_slice(&0.0f64.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&1u32.to_le_bytes());
        for &v in &fixture.lo {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for &v in &fixture.hi {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for _ in 0..m {
            out.extend_from_slice(&1.0f64.to_le_bytes());
        }
        for &v in &fixture.start_x {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for &v in &fixture.start_y {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let zero = 0.0f64.to_le_bytes();
        for _ in 0..side {
            out.extend_from_slice(&zero);
        }
        for _ in 0..side {
            out.extend_from_slice(&zero);
        }
        for _ in 0..side * side {
            out.extend_from_slice(&zero);
        }
        for _ in 0..side * side {
            out.extend_from_slice(&zero);
        }
        for _ in 0..6 * fixture.n {
            out.extend_from_slice(&zero);
        }
        out
    }
}
