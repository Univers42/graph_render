//! The subcommands of the Python-armed differentials: `emit-<name>-fixtures` and
//! `oracle-<name>`, flattened into the top-level command.

use super::{CLOSED_FORM, ENGINES, FA2, IGRAPH, SPECTRAL, TWOPI, emit, engine, ingest};
use crate::command::seed_count;
use clap::Subcommand;
use std::path::PathBuf;
use std::process::ExitCode;

/// The `--engine` values, read from the same table the dispatch does, so the parser and the
/// table cannot drift apart: a mistyped engine gets clap's error, which names the possibilities.
fn engine_names() -> impl clap::builder::TypedValueParser {
    clap::builder::PossibleValuesParser::new(
        ENGINES.iter().map(|engine| engine.name).collect::<Vec<_>>(),
    )
}

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
    /// Writes the twopi differential's fixtures for `harness/oracle-twopi.py`.
    ///
    /// The graph is the gate's own model, the one `emit-spectral-fixtures` writes too, so
    /// the fixtures Graphviz's engine is run over are the same fixtures the other
    /// differentials compare over.
    ///
    /// Kept as its own command: it is the spelling the twopi rows and the ADR use, and
    /// `oracle-graphviz --engine twopi` is the same emit under a general name.
    EmitTwopiFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long, default_value = "target/twopi-fixtures")]
        out: PathBuf,
    },
    /// Checks the twopi differential's result against its ceiling and records it.
    ///
    /// An alias for `oracle-graphviz --engine twopi`, kept so the rows, the measurements and
    /// the ADR keep the spelling they were written with.
    OracleTwopi {
        /// Directory holding the fixtures and `twopi-result.json`.
        #[arg(long, default_value = "target/twopi-fixtures")]
        dir: PathBuf,
    },
    /// Writes one Graphviz engine differential's fixtures for `harness/oracle-graphviz.py`.
    ///
    /// One command for every engine: the engine name picks the row, and with it the fixture
    /// stems, the ceiling and the ledger record. A new Graphviz engine is a new row, not a new
    /// command.
    EmitGraphvizFixtures {
        /// The Graphviz engine: `circo`, `twopi`.
        #[arg(long, value_parser = engine_names())]
        engine: String,
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Checks one Graphviz engine differential's result against its ceiling and records it.
    OracleGraphviz {
        /// The Graphviz engine: `circo`, `twopi`.
        #[arg(long, value_parser = engine_names())]
        engine: String,
        /// Directory holding the fixtures and `<engine>-result.json`.
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
            Cli::EmitTwopiFixtures { seeds, out } => emit(&TWOPI, seeds, None, &out),
            Cli::OracleTwopi { dir } => ingest(&TWOPI, &dir),
            Cli::EmitGraphvizFixtures { engine, seeds, out } => {
                engine_emit(&engine, seeds, out.unwrap_or_else(|| fixtures_dir(&engine)))
            }
            Cli::OracleGraphviz { engine, dir } => {
                engine_ingest(&engine, dir.unwrap_or_else(|| fixtures_dir(&engine)))
            }
        }
    }
}

/// `graph-cli emit-graphviz-fixtures --engine <name>`, refusing an engine there is no
/// differential for rather than emitting fixtures nothing will ever read.
fn engine_emit(name: &str, seeds: u32, out: PathBuf) -> ExitCode {
    match engine(name) {
        Ok(engine) => emit(&engine.differential, seeds, None, &out),
        Err(err) => {
            eprintln!("emit-graphviz-fixtures: {err}");
            ExitCode::from(2)
        }
    }
}

/// `graph-cli oracle-graphviz --engine <name>`.
fn engine_ingest(name: &str, dir: PathBuf) -> ExitCode {
    match engine(name) {
        Ok(engine) => ingest(&engine.differential, &dir),
        Err(err) => {
            eprintln!("oracle-graphviz: {err}");
            ExitCode::from(2)
        }
    }
}

/// The default fixture directory of an engine, `target/<engine>-fixtures`, named from the
/// same string the rest of the differential uses so there is one spelling of an engine.
fn fixtures_dir(name: &str) -> PathBuf {
    PathBuf::from(format!("target/{name}-fixtures"))
}
