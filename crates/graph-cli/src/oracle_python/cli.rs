//! The subcommands of the Python-armed differentials: `emit-<name>-fixtures` and
//! `oracle-<name>`, flattened into the top-level command.

use super::{FA2, IGRAPH, SPECTRAL, emit, ingest};
use crate::seed_count;
use clap::Subcommand;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Subcommand)]
pub enum Cli {
    /// Writes the spectral/pivot-MDS differential's fixtures for `harness/oracle-spectral.py`.
    EmitSpectralFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long, default_value = "target/spectral-fixtures")]
        out: PathBuf,
    },
    /// Checks the spectral differential's result against its ceilings and records it.
    OracleSpectral {
        /// Directory holding the fixtures and `spectral-result.json`.
        #[arg(long, default_value = "target/spectral-fixtures")]
        dir: PathBuf,
    },
    /// Writes the igraph-family differential's fixtures for `harness/oracle-igraph.py`.
    EmitIgraphFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 100, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long, default_value = "target/igraph-fixtures")]
        out: PathBuf,
    },
    /// Checks the igraph-family differential's result against its ceilings and records it.
    OracleIgraph {
        /// Directory holding the fixtures and `igraph-result.json`.
        #[arg(long, default_value = "target/igraph-fixtures")]
        dir: PathBuf,
    },
    /// Writes the ForceAtlas2 differential's fixtures for `harness/oracle-fa2.py`.
    EmitFa2Fixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Iteration budget both arms run, over the differential's own gated one. The
        /// escape hatch `docs/measurements/fa2-chaos.md` measures the chaos with.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
        max_iter: Option<u32>,
        /// Output directory.
        #[arg(long, default_value = "target/fa2-fixtures")]
        out: PathBuf,
    },
    /// Checks the ForceAtlas2 differential's result against its ceiling and records it.
    OracleFa2 {
        /// Directory holding the fixtures and `fa2-result.json`.
        #[arg(long, default_value = "target/fa2-fixtures")]
        dir: PathBuf,
    },
}

impl Cli {
    pub fn run(self) -> ExitCode {
        match self {
            Cli::EmitSpectralFixtures { seeds, out } => emit(&SPECTRAL, seeds, None, &out),
            Cli::OracleSpectral { dir } => ingest(&SPECTRAL, &dir),
            Cli::EmitIgraphFixtures { seeds, out } => emit(&IGRAPH, seeds, None, &out),
            Cli::OracleIgraph { dir } => ingest(&IGRAPH, &dir),
            Cli::EmitFa2Fixtures {
                seeds,
                max_iter,
                out,
            } => emit(&FA2, seeds, max_iter, &out),
            Cli::OracleFa2 { dir } => ingest(&FA2, &dir),
        }
    }
}
