//! graph-cli — the instruments around graph-core: the 4-way hash gate and its negative
//! control, the capabilities ledger, and the D1 determinism probe.
//!
//! Exit codes: `0` the check passed · `1` the check ran and failed (a gate went red) ·
//! `2` the check could not run (a tool, a file or an artifact was missing).

mod capabilities;
mod determinism_probe;
mod hashgate;
mod probe_report;
mod runner;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(name = "graph-cli", about = "graph-motor gates and ledger")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 4-way snapshot hash gate: native x2 and wasm32 x2 must agree on every seed.
    Hashgate {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 100)]
        seeds: u32,
    },
    /// One native arm of the gate, printing `stage seed sha256` lines. Spawned by `hashgate`.
    #[command(hide = true)]
    HashgateArm {
        /// Number of seeds, 0..N.
        #[arg(long)]
        seeds: u32,
    },
    /// The capabilities ledger, generated from the registry.
    Capabilities {
        /// Print every row as JSON.
        #[arg(long)]
        json: bool,
        /// Exit non-zero if any row claims more than its evidence supports.
        #[arg(long)]
        check: bool,
    },
    /// D1: std against libm transcendentals, native against wasm32, bit for bit.
    DeterminismProbe {
        /// Where to write the measurement, relative to the workspace root.
        #[arg(long, default_value = "docs/measurements/d1-ln1p.md")]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Hashgate { seeds } => hashgate::run(seeds),
        Command::HashgateArm { seeds } => hashgate::arm(seeds),
        Command::Capabilities { json, check } => capabilities::run(json, check),
        Command::DeterminismProbe { out } => determinism_probe::run(&out),
    }
}
