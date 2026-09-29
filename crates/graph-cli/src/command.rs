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
    Bench {
        /// Node counts, comma separated; `220,10000,100000` is the phase gate's set.
        #[arg(long, value_delimiter = ',', default_value = "220,10000,100000",
              value_parser = clap::value_parser!(u32).range(1..=i64::from(crate::bench::scale::MAX_SCALE_NODES)))]
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
        /// Phase 11: time the layout under each named execution tier (`scalar`, `threads`)
        /// instead of one run per size, and check every arm against the serial arm's
        /// bytes. `simd` and `gpu` are refused until those tiers exist.
        #[arg(long, value_delimiter = ',', value_parser = crate::bench::tiers::parse_asked_list())]
        tiers: Option<Vec<crate::bench::tiers::Asked>>,
        /// Phase 11: the worker counts `threads` is timed at.
        #[arg(long, value_delimiter = ',', default_value = crate::bench::tiers::WORKERS_DEFAULT,
              value_parser = clap::value_parser!(u32).range(1..))]
        workers: Vec<u32>,
    },
}
