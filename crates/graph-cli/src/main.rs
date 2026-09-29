//! graph-cli — the instruments around graph-core: the 4-way hash gate and its negative
//! controls, the oracle differential's fixtures, the capabilities ledger, the contract
//! codegen, the snapshot's two faces and their round trip, and the D1 determinism probe.
//!
//! Exit codes: `0` the check passed · `1` the check ran and failed (a gate went red) ·
//! `2` the check could not run (a tool, a file or an artifact was missing).

mod bench;
mod capabilities;
mod codegen;
mod determinism_probe;
mod evidence;
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
        #[arg(long, default_value_t = 100, value_parser = seed_count())]
        seeds: u32,
    },
    /// One native arm of the gate, printing `stage seed sha256` lines. Spawned by `hashgate`.
    #[command(hide = true)]
    HashgateArm {
        /// Number of seeds, 0..N.
        #[arg(long, value_parser = seed_count())]
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
        /// Phase 9: also check docs/measurements/phase09-ceilings.md, the before/after
        /// table of every declared `scale_ceiling` against what was measured.
        #[arg(long)]
        ceilings_measured: bool,
    },
    /// Writes the contract's JSON Schema and TypeScript to their committed files.
    Codegen {
        /// Write nothing; exit 1 if a committed file is stale.
        #[arg(long)]
        check: bool,
    },
    /// The ingest contract to a graph: `graph_core::ingest::build`, the one derivation,
    /// on the command line. `--check` compares against a committed file instead of
    /// writing, so the same command regenerates a fixture and gates it.
    Ingest {
        /// A JSON document holding the contract, or an object with it as a member.
        #[arg(long)]
        from: PathBuf,
        /// Which member of that document is the contract. `ingest` for the committed
        /// convergence fixture, which keeps the derived graph beside it.
        #[arg(long, default_value = "ingest")]
        member: String,
        /// Where the derived graph goes; standard output when absent.
        #[arg(long, conflicts_with = "check")]
        out: Option<PathBuf>,
        /// Compare against this file and exit 1 if it differs, writing nothing. The
        /// same command that regenerates a fixture and the one that gates it.
        #[arg(long, value_name = "PATH")]
        check: Option<PathBuf>,
    },
    /// Writes the oracle differential's cases and graph-core's expected outputs.
    EmitFixtures {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 1000, value_parser = seed_count())]
        seeds: u32,
        /// Output directory; `target/oracle-fixtures` by default.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Runs `harness/oracle-diff.mjs` over the emitted fixtures (the TypeScript arm).
    OracleDiff {
        /// Fixtures directory; `target/oracle-fixtures` by default.
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
    #[command(flatten)]
    PythonOracle(oracle_python::Cli),
    /// Runs `harness/oracle-layouts.mjs` over the emitted fixtures (the d3-hierarchy arm).
    OracleLayouts {
        /// Fixtures directory; `target/oracle-fixtures` by default.
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
    /// Runs one seed's model through a layout and writes the snapshot's binary face, its
    /// canonical JSON face, or both. A summary goes to standard error.
    Snapshot {
        /// Seed of the synthetic model.
        #[arg(long)]
        seed: u32,
        /// Node count; by default the hash gate's for this seed, 2 + seed % 600.
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=snapshot_cmd::MAX_NODES))]
        nodes: Option<u32>,
        /// Registered layout.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(snapshot_cmd::layout_names()))]
        layout: String,
        /// Where to write the binary face; `-` for standard output.
        #[arg(long, value_name = "PATH", required_unless_present = "out_json")]
        out_bin: Option<PathBuf>,
        /// Where to write the canonical JSON face; `-` for standard output.
        #[arg(long, value_name = "PATH")]
        out_json: Option<PathBuf>,
    },
    /// Binary <-> JSON round trip over seeds 0..N, byte-exact, plus the grid's hand oracle.
    Roundtrip {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 100, value_parser = seed_count())]
        seeds: u32,
    },
    /// Ink saved by every registered bundler on a POST fixture, or the wall time of one
    /// pass on the hairball generator at `--nodes` (the `scale_ceiling` sweep). Exit 1 when a
    /// bundler did not reduce the occupied cells.
    Ink {
        /// A committed POST fixture, by name: `hairball` or `long-span`.
        #[arg(long, conflicts_with = "nodes", required_unless_present = "nodes")]
        fixture: Option<String>,
        /// The hairball generator's node count.
        #[arg(long, value_parser = clap::value_parser!(u32).range(2..=100_000))]
        nodes: Option<u32>,
        /// The layout that draws the graph first.
        #[arg(long, default_value = "circular.radial")]
        layout: String,
    },
    /// D1: std against libm transcendentals, native against wasm32, bit for bit.
    DeterminismProbe {
        /// Where to write the measurement, relative to the workspace root.
        #[arg(long, default_value = "docs/measurements/d1-ln1p.md")]
        out: PathBuf,
    },
    /// Force-layout quality: our stress correlation against a real d3-force simulation
    /// of the same graph, under the frozen margin.
    Stress {
        /// The oracle to compare against; `d3` is the only one wired. Required, not
        /// defaulted: a quality gate that silently picked its own baseline would be a
        /// gate comparing the implementation against itself.
        #[arg(long)]
        oracle: String,
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 8, value_parser = seed_count())]
        seeds: u32,
    },
    /// Wall time and Kruskal stress-1 of the Phase 6 layouts (or `--layout`) at the given
    /// node counts, refusing a size past a layout's own registered `scale_ceiling`.
    Bench {
        /// Node counts, comma separated; `220,10000,100000` is the phase gate's set.
        #[arg(long, value_delimiter = ',', default_value = "220,10000,100000",
              value_parser = clap::value_parser!(u32).range(1..=i64::from(bench::scale::MAX_SCALE_NODES)))]
        n: Vec<u32>,
        /// Registered layout ids; repeat for several. Default: the Phase 6 layouts.
        #[arg(long)]
        layout: Vec<String>,
        /// Seed of the synthetic model.
        #[arg(long, default_value_t = 0)]
        seed: u32,
        /// Run sizes past a layout's `scale_ceiling` too, labelled as such.
        #[arg(long)]
        past_ceiling: bool,
        /// Also time the d3-force arm (`harness/stress-d3.mjs`) on the same graph, for
        /// `layout.force.barnes_hut`.
        #[arg(long)]
        vs_d3: bool,
        /// Report which sizes each layout would run or refuse, and run none of them.
        #[arg(long)]
        dry_run: bool,
        /// Phase 9: runs per cell. The campaign reports the median, never one timing.
        #[arg(long, default_value_t = 5)]
        repeat: u32,
        /// Phase 9: write the campaign's markdown here.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Phase 9: report the largest N per arm that fits the frame budget.
        #[arg(long)]
        crossover: bool,
        /// Phase 9: the frame budget in milliseconds (`prompt.md` §5.2: 16.67).
        #[arg(long, default_value_t = crate::bench::campaign::FRAME_BUDGET_MS)]
        budget_ms: f64,
        /// Phase 9: write the scale fixture for `--n` and `--seed` here and measure
        /// nothing. The generator is the artefact; the file is one sample of it.
        #[arg(long, value_name = "PATH")]
        emit_scale_fixture: Option<PathBuf>,
    },
}

/// Most seeds one gate run may ask for. Every seed is four child computations; past this
/// a typo (`--seeds 1000000000`) would look like a hang rather than an error.
const MAX_SEEDS: i64 = 100_000;

fn seed_count() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(0..=MAX_SEEDS)
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Hashgate { seeds } => hashgate::run(seeds),
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
        Command::DeterminismProbe { out } => determinism_probe::run(&out),
        Command::Stress { oracle, seeds } => stress::run(&oracle, seeds),
        Command::Bench {
            n,
            layout,
            seed,
            past_ceiling,
            vs_d3,
            dry_run,
            repeat,
            out,
            crossover,
            budget_ms,
            emit_scale_fixture,
        } => bench::run(&bench::Plan {
            sizes: n,
            layouts: layout,
            seed,
            past_ceiling,
            vs_d3,
            dry_run,
            repeat,
            out,
            crossover,
            budget_ms,
            emit_scale_fixture,
        }),
    }
}
