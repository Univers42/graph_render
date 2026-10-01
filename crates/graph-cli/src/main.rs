//! graph-cli — the instruments around graph-core: the 4-way hash gate and its negative
//! controls, the oracle differential's fixtures, the capabilities ledger, the contract
//! codegen, the snapshot's two faces and their round trip, and the D1 determinism probe.
//!
//! Exit codes: `0` the check passed · `1` the check ran and failed (a gate went red) ·
//! `2` the check could not run (a tool, a file or an artifact was missing).

mod bench;
mod capabilities;
mod codegen;
mod command;
mod determinism_probe;
mod evidence;
mod exec_native;
mod fingerprint;
mod hashgate;
mod ingest_cmd;
mod ink_cmd;
mod oracle_fixtures;
mod oracle_python;
mod probe_report;
mod runner;
mod snapshot_cmd;
mod stress;

use clap::Parser;
use std::process::ExitCode;

use command::Command;

#[derive(Parser)]
#[command(name = "graph-cli", about = "graph-motor gates and ledger")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Hashgate { seeds, tiers } => hashgate::run(seeds, tiers),
        Command::HashgateArm { seeds } => hashgate::arm(seeds),
        Command::Capabilities {
            json,
            check,
            ceilings_measured,
        } => capabilities::run(json, check, ceilings_measured),
        Command::Codegen { check } => codegen::run(check),
        Command::Ingest {
            from,
            member,
            out,
            check,
        } => {
            let mode = if check.is_some() {
                ingest_cmd::Mode::Check
            } else {
                ingest_cmd::Mode::Write
            };
            ingest_cmd::run(&ingest_cmd::Plan {
                from,
                member,
                out: check.or(out),
                mode,
            })
        }
        Command::EmitFixtures { seeds, out } => {
            oracle_fixtures::run(seeds, &out.unwrap_or_else(oracle_fixtures::default_out))
        }
        Command::OracleDiff { fixtures } => {
            oracle_fixtures::diff(&fixtures.unwrap_or_else(oracle_fixtures::default_out))
        }
        Command::EmitSpectralFixtures { seeds, out } => {
            oracle_python::emit(&oracle_python::SPECTRAL, seeds, None, &out)
        }
        Command::OracleSpectral { dir } => oracle_python::ingest(&oracle_python::SPECTRAL, &dir),
        Command::EmitFa2Fixtures {
            seeds,
            max_iter,
            out,
        } => oracle_python::emit(&oracle_python::FA2, seeds, max_iter, &out),
        Command::OracleFa2 { dir } => oracle_python::ingest(&oracle_python::FA2, &dir),
        Command::EmitClosedFormFixtures { seeds, out } => {
            oracle_python::emit(&oracle_python::CLOSED_FORM, seeds, None, &out)
        }
        Command::OracleClosedForm { dir } => {
            oracle_python::ingest(&oracle_python::CLOSED_FORM, &dir)
        }
        Command::EmitSpringFixtures {
            seeds,
            max_iter,
            out,
        } => oracle_python::emit(&oracle_python::SPRING, seeds, max_iter, &out),
        Command::OracleSpring { dir } => oracle_python::spring::ingest::ingest(&dir),
        Command::EmitCircularHierarchyFixtures { seeds, out } => {
            oracle_python::emit(&oracle_python::CIRCULAR_HIERARCHY, seeds, None, &out)
        }
        Command::OracleCircularHierarchy { dir } => {
            oracle_python::ingest(&oracle_python::CIRCULAR_HIERARCHY, &dir)
        }
        Command::OracleLayouts { fixtures } => {
            oracle_fixtures::diff_layouts(&fixtures.unwrap_or_else(oracle_fixtures::default_out))
        }
        Command::Snapshot {
            seed,
            nodes,
            layout,
            out_bin,
            out_json,
        } => {
            let out = snapshot_cmd::Outputs {
                bin: out_bin,
                json: out_json,
            };
            snapshot_cmd::snapshot(seed, nodes, &layout, &out)
        }
        Command::Roundtrip { seeds } => snapshot_cmd::roundtrip(seeds),
        Command::Ink {
            fixture,
            nodes,
            layout,
        } => ink_cmd::run(&ink_cmd::Request {
            fixture: fixture.as_deref(),
            nodes,
            layout: &layout,
        }),
        Command::DeterminismProbe { out } => determinism_probe::run(&out),
        Command::Stress { oracle, seeds } => stress::run(&oracle, seeds),
        Command::Bench(plan) => bench::run(&plan),
    }
}
