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
mod forcecheck;
mod hashgate;
mod ingest_cmd;
mod ink_cmd;
mod mb_fidelity;
mod oracle_fixtures;
mod oracle_python;
mod overlap_cmd;
mod probe_report;
mod runner;
mod snapshot_cmd;
mod stream_fixtures;
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
        Command::ForceGate { seeds } => forcecheck::run(seeds),
        Command::ForceGateArm { seeds } => forcecheck::arm(seeds),
        Command::ForceGateStreamArm => forcecheck::stream::arm(),
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
        Command::EmitStreamFixtures { out } => {
            let out = out.unwrap_or_else(stream_fixtures::default_out);
            match stream_fixtures::run(&out) {
                Ok(()) => ExitCode::SUCCESS,
                Err(err) => {
                    eprintln!("emit-stream-fixtures: {err}");
                    ExitCode::from(2)
                }
            }
        }
        Command::EmitFixtures { seeds, out } => {
            oracle_fixtures::run(seeds, &out.unwrap_or_else(oracle_fixtures::default_out))
        }
        Command::OracleDiff { fixtures } => {
            oracle_fixtures::diff(&fixtures.unwrap_or_else(oracle_fixtures::default_out))
        }
        Command::PythonOracle(command) => command.run(),
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
        Command::Overlap {
            fixture,
            nodes,
            layout,
            radius,
            no_scan,
            max_iterations,
        } => overlap_cmd::run(&overlap_cmd::Request {
            fixture: fixture.as_deref(),
            nodes,
            layout: &layout,
            radius,
            scan: !no_scan,
            max_iterations,
        }),
        Command::DeterminismProbe { out } => determinism_probe::run(&out),
        Command::Stress {
            oracle,
            layout,
            seeds,
        } => stress::run(&oracle, &layout, seeds),
        Command::Bench(plan) => bench::run(&plan),
        Command::Tick(plan) => bench::tick::run(&plan),
        Command::MbFidelity(plan) => mb_fidelity::run(&plan),
    }
}
