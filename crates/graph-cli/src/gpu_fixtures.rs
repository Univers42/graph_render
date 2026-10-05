//! `graph-cli emit-gpu-fixtures`: the particle mesh's own per-pass velocity increments, at
//! the start state and 100 ticks in, for the GPU arm to load unchanged.
//!
//! One generator, one file format, one loader (`crates/graph-sdk-js/src/gpu/fixture.ts`):
//! the repo's oracle rule. This crate writes, the browser reads, and neither re-implements
//! the other's arithmetic.
//!
//! Every value written is the CPU's own `f64` and every integer is `u32` (D6). No section is
//! recomputed here: the frame, the twiddles and the kernel spectrum come out of
//! `ForceSession::mesh_probe`, which reads them out of the mesh's own solve.
//!
//! Caveat: the emit is a *measurement's* generator, not a build step. It runs [`settle::case`]
//! at every size, and at 1M that is 100 ticks of a 1024-side mesh before a byte is written —
//! minutes, not seconds. Nothing here is on any product path.

mod check;
mod emit;
mod settle;

#[cfg(test)]
#[path = "gpu_fixtures/tests.rs"]
mod tests;

use clap::Subcommand;
use std::path::PathBuf;
use std::process::ExitCode;

pub use settle::State;

/// `emit-gpu-fixtures`, one subcommand with the two halves of the same job: write the
/// files, or byte-compare the committed ones against what this tree would write.
#[derive(Subcommand)]
pub enum Cli {
    /// Writes the mesh fixtures for the GPU arm, or byte-compares the committed ones.
    #[command(name = "emit-gpu-fixtures")]
    EmitGpuFixtures {
        /// Output directory.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Byte-compare against this directory instead of writing.
        #[arg(long, value_name = "PATH")]
        check: Option<PathBuf>,
    },
}

impl Cli {
    /// Writes every case under `--out`, or compares every case against `--check`. Neither
    /// flag, or both, is exit 2: there is no default directory to guess and a wrong guess
    /// would write derived binaries somewhere nobody looks.
    pub fn run(self) -> ExitCode {
        match self {
            Cli::EmitGpuFixtures { out: Some(dir), check: None } => write_all(&dir),
            Cli::EmitGpuFixtures { out: None, check: Some(dir) } => check::all(&dir),
            Cli::EmitGpuFixtures { out: _, check: _ } => ExitCode::from(2),
        }
    }
}

/// Writes every case into `dir` and prints each file's real byte count **and** the frame the
/// mesh placed for it, so a file far from the README's estimate and a frame far from the
/// documented ladder are both visible in this run's own output rather than in a later
/// failure.
fn write_all(dir: &std::path::Path) -> ExitCode {
    let mut knobs = emit::Knobs::from_env();
    if let Err(err) = std::fs::create_dir_all(dir) {
        eprintln!("emit-gpu-fixtures: {}: {err}", dir.display());
        return ExitCode::from(2);
    }
    for n in settle::SIZES {
        for state in State::all() {
            let name = settle::file_name(n, state);
            match one(n, state, &mut knobs) {
                Ok(Case { bytes, line }) => match std::fs::write(dir.join(&name), &bytes) {
                    Ok(()) => println!("emit-gpu-fixtures: {name}  {line}"),
                    Err(err) => {
                        eprintln!("emit-gpu-fixtures: writing {name}: {err}");
                        return ExitCode::from(2);
                    }
                },
                Err(err) => {
                    eprintln!("emit-gpu-fixtures: {name}: {err}");
                    return ExitCode::from(2);
                }
            }
        }
    }
    ExitCode::SUCCESS
}

/// One case's bytes and the one line of numbers the run prints for it.
struct Case {
    bytes: Vec<u8>,
    line: String,
}

/// One case: settle to its state, probe it, write it.
fn one(n: u32, state: State, knobs: &mut emit::Knobs) -> Result<Case, String> {
    let (session, probe) = settle::case(n, state)?;
    let bytes = emit::write(&probe, session.xs(), session.ys(), state, knobs)?;
    let m = probe.lo.len();
    let line = format!(
        "{} bytes  P={}  m={}  step={}  h={}  origin=({}, {})  cells={}  reach={}  scale={:?}",
        bytes.len(),
        probe.side,
        m,
        probe.step,
        probe.h,
        probe.origin_x,
        probe.origin_y,
        probe.cells,
        probe.reach,
        emit::scale_for(n)
    );
    Ok(Case { bytes, line })
}

