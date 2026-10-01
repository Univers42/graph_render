//! The subcommands of the Python-armed differentials: `emit-<name>-fixtures` and
//! `oracle-<name>`, flattened into the top-level command.

use super::{CLOSED_FORM, FA2, IGRAPH, NEATO, SPECTRAL, TWOPI, emit, ingest};
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
    /// Writes the twopi differential's fixtures for `harness/oracle-twopi.py`.
    ///
    /// The graph is the gate's own model, the one `emit-spectral-fixtures` writes too, so
    /// the fixtures Graphviz's engine is run over are the same fixtures the other
    /// differentials compare over.
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
    /// The same differential as `oracle-graphviz --engine twopi`, kept as its own
    /// subcommand so the p13-gv1 gate row and any run recorded against it still work. Both
    /// arms of a twopi differential are byte for byte, so `--engine` is not a choice anyone
    /// needs to make for that one; this engine is a force engine and its differential is.
    OracleTwopi {
        /// Directory holding the fixtures and `twopi-result.json`.
        #[arg(long, default_value = "target/twopi-fixtures")]
        dir: PathBuf,
    },
    /// Writes a Graphviz engine differential's fixtures for `harness/oracle-graphviz.py`.
    ///
    /// The graph is the gate's own model, the one `emit-spectral-fixtures` writes too, so the
    /// fixtures the engine is run over are the same fixtures the other differentials compare
    /// over.
    EmitGraphvizFixtures {
        /// Which engine. One differential per engine, and an unknown name is a usage error
        /// rather than a silently empty comparison.
        #[arg(long, value_parser = graphviz_engine())]
        engine: String,
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory; defaults to the engine's own, so `--engine neato` and
        /// `--dir target/neato-fixtures` cannot name two different places.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Checks a Graphviz engine differential's result against its ceiling and records it.
    /// `oracle-twopi` is not an alias here: it is the older, still-working subcommand above,
    /// and clap refuses an alias that collides with a real command's name. The generalisation
    /// is `oracle-graphviz --engine twopi`, and the old spelling stays as itself.
    OracleGraphviz {
        /// Which engine, as above.
        #[arg(long, value_parser = graphviz_engine())]
        engine: String,
        /// Directory holding the engine's fixtures and `<engine>-result.json`; defaults to
        /// the engine's own, so the two halves of the chain cannot name different places.
        #[arg(long)]
        dir: Option<PathBuf>,
    },
}

/// The `--engine` values a Graphviz differential exists for, as `(name, fixture directory)`.
///
/// The directory is derived from the engine name rather than taken as a second flag, so the
/// two cannot disagree: `neato`'s fixtures live in `target/neato-fixtures` and there is no
/// way to ask for one engine's fixtures in another's directory. An unknown engine is a
/// usage error at parse time, which is the point — a differential silently run over nothing
/// is the failure this rules out.
const GRAPHVIZ_ENGINES: [(&str, &str); 2] = [
    ("neato", "target/neato-fixtures"),
    ("twopi", "target/twopi-fixtures"),
];

/// A `--engine` value that names a differential this build has.
///
/// A `PossibleValuesParser` over the static table, so clap rejects an unknown engine itself
/// and prints the ones that exist. The table is the single source of truth for the accepted
/// values, the default directories and the differential lookup below: adding an engine is
/// one row here plus one `Differential`, not three edits that can drift apart.
fn graphviz_engine() -> impl clap::builder::TypedValueParser<Value = String> {
    clap::builder::PossibleValuesParser::new(
        GRAPHVIZ_ENGINES
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
    )
}

/// The fixture directory an engine's `--dir` defaults to, or the given one.
fn graphviz_dir(engine: &str, dir: Option<PathBuf>) -> PathBuf {
    dir.unwrap_or_else(|| {
        GRAPHVIZ_ENGINES
            .iter()
            .find(|(name, _)| *name == engine)
            .map(|(_, path)| PathBuf::from(path))
            .expect("clap validated the engine")
    })
}

/// The differential an `--engine` names, or a usage error naming the ones that exist.
fn graphviz_differential(engine: &str) -> Result<&'static super::Differential, String> {
    match engine {
        "neato" => Ok(&NEATO),
        "twopi" => Ok(&TWOPI),
        other => Err(format!(
            "{other}: no Graphviz differential; known engines are neato, twopi"
        )),
    }
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
                match graphviz_differential(&engine) {
                    Ok(differential) => {
                        emit(differential, seeds, None, &graphviz_dir(&engine, out))
                    }
                    Err(err) => {
                        eprintln!("emit-graphviz-fixtures: {err}");
                        ExitCode::from(2)
                    }
                }
            }
            Cli::OracleGraphviz { engine, dir } => match graphviz_differential(&engine) {
                Ok(differential) => ingest(differential, &graphviz_dir(&engine, dir)),
                Err(err) => {
                    eprintln!("oracle-graphviz: {err}");
                    ExitCode::from(2)
                }
            },
        }
    }
}
