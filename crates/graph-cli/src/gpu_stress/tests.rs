//! `gpu-stress`'s own checks: the plumbing end to end, and the reader's refusals.

use super::reader::{Fixture, MAGIC, MAJOR, Positions};
use super::*;
use graph_core::layout::force::{ForceParams, ForceSession, LiveParams};
use std::path::Path;

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
        strength: vec![1.0; lo.len()],
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
    let (ratio, displacement) = compare(&plan).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        (ratio - 1.0).abs() < 1e-6,
        "the CPU compared against itself is {ratio}"
    );
    assert_eq!(
        displacement, 0.0,
        "the CPU's own positions are no distance from themselves"
    );
}

/// At one tick the displacement is held; over many, only the ratio. A NaN fails both.
#[test]
fn one_tick_holds_the_displacement_and_many_do_not() {
    assert!(passes(1, 1.0, 0.0));
    assert!(!passes(1, 1.0, DISPLACEMENT_CEILING * 2.0));
    assert!(passes(300, 1.0, 0.5));
    assert!(!passes(300, 2.5, 0.0));
    assert!(!passes(1, 1.0, f64::NAN));
    assert!(!passes(300, f64::NAN, 0.0));
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
    for &v in &fixture.strength {
        out.extend_from_slice(&v.to_le_bytes());
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
