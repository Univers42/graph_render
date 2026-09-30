//! Command enum and helper parsers for graph-cli.

use clap::{Subcommand, builder::TypedValueParser};
use std::path::PathBuf;

use crate::hashgate;

/// Most seeds one gate run may ask for. Every seed is four child computations; past this
/// a typo (`--seeds 1000000000`) would look like a hang rather than an error.
pub const MAX_SEEDS: i64 = 100_000;

pub fn seed_count() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(0..=MAX_SEEDS)
}

/// `--tiers`, parsed by `hashgate`'s own list so the flag and the arm list cannot drift.
///
/// `PossibleValuesParser` over the accepted words and `try_map` into the enum: the value
/// parser vets the word, and the map is a lookup in the same two-entry list, so the error a
/// mistyped word gets is clap's (which names the possibilities) rather than a second
/// hand-written message.
pub fn parse_tiers() -> impl TypedValueParser {
    clap::builder::PossibleValuesParser::new(["base", "all"])
        .try_map(|word| hashgate::parse_tiers(word.as_str()))
}

#[derive(Subcommand)]
pub enum Command {
    /// The snapshot hash gate: every arm must agree on every seed, per stage.
    Hashgate {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 100, value_parser = seed_count())]
        seeds: u32,
        /// Which arms to run: `base` is native x2 and wasm32 x2, `all` adds one native arm
        /// per thread count in {1, 2, 3, 4, 7}. The odd counts are the point — an even
        /// split hides a range-boundary bug.
        #[arg(long, default_value = "base", value_parser = parse_tiers())]
        tiers: hashgate::Tiers,
    },
    /// One native arm of the gate, printing `stage seed sha256` lines. Spawned by `hashgate`.
    #[command(hide = true)]
    HashgateArm {
        /// Number of seeds, 0..N.
        #[arg(long, value_parser = seed_count())]
        seeds: u32,
    },
    /// The live force session's own hash gate: native ×2 against wasm32 ×2 over the positions
    /// after a fixed number of ticks, driven through `gm_force_session_*`. See
    /// `docs/decisions/force-wasm-abi.md`.
    ForceGate {
        /// Number of seeds, 0..N.
        #[arg(long, default_value_t = 4, value_parser = seed_count())]
        seeds: u32,
    },
    /// One native arm of the force gate, printing `stage seed sha256` lines. Spawned by
    /// `force-gate`.
    #[command(hide = true)]
    ForceGateArm {
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
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=crate::snapshot_cmd::MAX_NODES))]
        nodes: Option<u32>,
        /// Registered layout.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(crate::snapshot_cmd::layout_names()))]
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
    Bench(crate::bench::Plan),
}
