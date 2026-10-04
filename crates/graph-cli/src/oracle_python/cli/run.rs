//! Dispatch of the Python-armed subcommands. Split from `cli.rs` by the house's 300-line limit.

use super::Cli;
use crate::oracle_python::graphviz::{by_engine, default_dir};
use crate::oracle_python::spring;
use crate::oracle_python::{
    BASIC_3D, CIRCULAR_HIERARCHY, CLOSED_FORM, FA2, HIERARCHICAL_3D, IGRAPH, IGRAPH_3D, SCALE,
    SPECTRAL, SPRING, conformance, emit, ingest,
};
use std::process::ExitCode;

impl Cli {
    pub fn run(self) -> ExitCode {
        match self {
            Cli::EmitSpectralFixtures { seeds, out } => emit(&SPECTRAL, seeds, None, &out),
            Cli::OracleSpectral { dir } => ingest(&SPECTRAL, &dir),
            Cli::EmitIgraphFixtures { seeds, out } => emit(&IGRAPH, seeds, None, &out),
            Cli::OracleIgraph { dir } => ingest(&IGRAPH, &dir),
            Cli::EmitIgraph3dFixtures { seeds, out } => emit(&IGRAPH_3D, seeds, None, &out),
            Cli::OracleIgraph3d { dir } => ingest(&IGRAPH_3D, &dir),
            Cli::EmitFa2Fixtures {
                seeds,
                max_iter,
                out,
            } => emit(&FA2, seeds, max_iter, &out),
            Cli::OracleFa2 { dir } => ingest(&FA2, &dir),
            Cli::EmitClosedFormFixtures { seeds, out } => emit(&CLOSED_FORM, seeds, None, &out),
            Cli::OracleClosedForm { dir } => ingest(&CLOSED_FORM, &dir),
            Cli::EmitBasic3dFixtures { seeds, out } => emit(&BASIC_3D, seeds, None, &out),
            Cli::OracleBasic3d { dir } => ingest(&BASIC_3D, &dir),
            Cli::EmitHierarchical3dFixtures { seeds, out } => {
                emit(&HIERARCHICAL_3D, seeds, None, &out)
            }
            Cli::OracleHierarchical3d { dir } => ingest(&HIERARCHICAL_3D, &dir),
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
            Cli::EmitScaleFixtures { cases, out } => emit(&SCALE, cases, None, &out),
            Cli::OracleScale { dir } => ingest(&SCALE, &dir),
            Cli::EmitConformanceFixtures { out } => conformance::emit(&out),
            Cli::ScigraphsConformance { dir } => conformance::judge(&dir),
        }
    }
}

/// An engine name the parser should already have refused. Exit 2, the code the other
/// "could not run" arms use, so a mistyped engine is never read as a pass.
fn unknown(engine: &str) -> ExitCode {
    eprintln!("oracle-graphviz: no differential for engine {engine}");
    ExitCode::from(2)
}
