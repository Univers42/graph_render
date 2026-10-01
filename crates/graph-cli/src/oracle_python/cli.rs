//! The subcommands of the Python-armed differentials: `emit-<name>-fixtures` and
//! `oracle-<name>`, flattened into the top-level command.

use super::graphviz::{by_engine, default_dir, engine_parser};
use super::spring;
use super::{CIRCULAR_HIERARCHY, CLOSED_FORM, FA2, IGRAPH, SPECTRAL, SPRING, emit, ingest};
use crate::command::seed_count;
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
    /// Writes the closed-form differential's fixtures for `harness/oracle-closed-form.py`.
    EmitClosedFormFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long, default_value = "target/closed-form-fixtures")]
        out: PathBuf,
    },
    /// Checks the closed-form differential's result against its ceiling and records it.
    OracleClosedForm {
        /// Directory holding the fixtures and `closed-form-result.json`.
        #[arg(long, default_value = "target/closed-form-fixtures")]
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
    /// Writes the spring differential's fixtures for `harness/oracle-spring.py`.
    EmitSpringFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Iteration budget both arms run, over the differential's own. The escape hatch
        /// `docs/measurements/p12-t2.md` measures another budget with.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=1000))]
        max_iter: Option<u32>,
        /// Output directory.
        #[arg(long, default_value = "target/spring-fixtures")]
        out: PathBuf,
    },
    /// Checks the spring differential's result against its stress-ratio ceiling.
    OracleSpring {
        /// Directory holding the fixtures and `spring-result.json`.
        #[arg(long, default_value = "target/spring-fixtures")]
        dir: PathBuf,
    },
    /// Writes the circular-hierarchy differential's fixtures for
    /// `harness/oracle-circular-hierarchy.py`, the SciGraphs arm.
    EmitCircularHierarchyFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long, default_value = "target/circular-hierarchy-fixtures")]
        out: PathBuf,
    },
    /// Checks the circular-hierarchy differential's result against its ceiling.
    OracleCircularHierarchy {
        /// Directory holding the fixtures and `circular-hierarchy-result.json`.
        #[arg(long, default_value = "target/circular-hierarchy-fixtures")]
        dir: PathBuf,
    },
    /// Writes one Graphviz engine differential's fixtures for `harness/oracle-graphviz.py`.
    ///
    /// The graph is the gate's own model, the one `emit-spectral-fixtures` writes too, so
    /// the fixtures Graphviz's engine is run over are the same fixtures the other
    /// differentials compare over.
    ///
    /// The alias is the command this replaced: `emit-twopi-fixtures` with no `--engine` is
    /// this command with `--engine twopi`, writing the same `target/twopi-fixtures`.
    #[command(alias = "emit-twopi-fixtures")]
    EmitGraphvizFixtures {
        /// Which Graphviz engine to compare against.
        #[arg(long, default_value = "twopi", value_parser = engine_parser())]
        engine: String,
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory, `target/<engine>-fixtures` when unset.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Checks one Graphviz engine differential's result against its ceiling and records it.
    ///
    /// The alias is the command this replaced: `oracle-twopi` with no `--engine` is this
    /// command with `--engine twopi`, over the same directory and against the same record.
    #[command(alias = "oracle-twopi")]
    OracleGraphviz {
        /// Which Graphviz engine's differential to check.
        #[arg(long, default_value = "twopi", value_parser = engine_parser())]
        engine: String,
        /// Directory holding the fixtures and `<engine>-result.json`,
        /// `target/<engine>-fixtures` when unset.
        #[arg(long)]
        dir: Option<PathBuf>,
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
            Cli::EmitClosedFormFixtures { seeds, out } => emit(&CLOSED_FORM, seeds, None, &out),
            Cli::OracleClosedForm { dir } => ingest(&CLOSED_FORM, &dir),
            Cli::EmitSpringFixtures {
                seeds,
                max_iter,
                out,
            } => emit(&SPRING, seeds, max_iter, &out),
            Cli::OracleSpring { dir } => spring::ingest::ingest(&dir),
            Cli::EmitCircularHierarchyFixtures { seeds, out } => {
                emit(&CIRCULAR_HIERARCHY, seeds, None, &out)
            }
            Cli::OracleCircularHierarchy { dir } => ingest(&CIRCULAR_HIERARCHY, &dir),
            Cli::EmitGraphvizFixtures { engine, seeds, out } => match by_engine(&engine) {
                Some(differential) => emit(
                    &differential,
                    seeds,
                    None,
                    &out.unwrap_or(default_dir(&engine)),
                ),
                None => unknown(&engine),
            },
            Cli::OracleGraphviz { engine, dir } => match by_engine(&engine) {
                Some(differential) => ingest(&differential, &dir.unwrap_or(default_dir(&engine))),
                None => unknown(&engine),
            },
        }
    }
}

/// An engine name the parser should already have refused. Exit 2, the code the other
/// "could not run" arms use, so a mistyped engine is never read as a pass.
fn unknown(engine: &str) -> ExitCode {
    eprintln!("oracle-graphviz: no differential for engine {engine}");
    ExitCode::from(2)
}
