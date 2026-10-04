//! Command enum and helper parsers for graph-cli.

use clap::{Subcommand, builder::TypedValueParser};
use std::path::PathBuf;

use crate::hashgate;

/// Most seeds one gate run may ask for. Every seed is four child computations; past this
/// a typo (`--seeds 1000000000`) would look like a hang rather than an error.
pub const MAX_SEEDS: i64 = 100_000;

/// The fewest seeds a gate row may ask for. Zero is refused, not accepted: a gate over
/// no seed runs its loop zero times, prints nothing and exits 0 — a comparison of
/// nothing that reads as a pass.
pub const MIN_SEEDS: i64 = 1;

/// `--seeds`, for every subcommand that counts seeds: the floor and the ceiling are the two
/// constants above, so the message clap prints and the range it enforces cannot drift apart.
pub fn seed_count() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(MIN_SEEDS..=MAX_SEEDS)
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
        /// Number of seeds, 1..N.
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
        /// Number of seeds, 1..N.
        #[arg(long, value_parser = seed_count())]
        seeds: u32,
    },
    /// The live force session's own hash gate: native ×2 against wasm32 ×2 over the positions
    /// after a fixed number of ticks, driven through `gm_force_session_*`. See
    /// `docs/decisions/force-wasm-abi.md`.
    ForceGate {
        /// Number of seeds, 1..N.
        #[arg(long, default_value_t = 4, value_parser = seed_count())]
        seeds: u32,
    },
    /// One native arm of the force gate, printing `stage seed sha256` lines. Spawned by
    /// `force-gate`.
    #[command(hide = true)]
    ForceGateArm {
        /// Number of seeds, 1..N.
        #[arg(long, value_parser = seed_count())]
        seeds: u32,
    },
    /// One native arm of the force gate's **stream** stage, printing
    /// `force.session.stream <fixture> <batch> <sha256>` a line per batch. Spawned by
    /// `force-gate`.
    ///
    /// Takes no seed count: the stream fixtures are three fixed files rather than a seed
    /// family, so a `--seeds` here would be a number nothing reads.
    #[command(hide = true)]
    ForceGateStreamArm,
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
        /// Which member of that document is the contract; `ingest` for the committed
        /// convergence fixture, which keeps the derived graph beside it. Stated, never
        /// defaulted, on both paths: a defaulted key made `ingest --check` parse `ingest`
        /// out of a fixture whose contract sits under another member and report that
        /// (empty) parse as a comparison. clap 4 has no "required if `--check` is present"
        /// (no `required_if_present`), so the floor is the whole subcommand; both paths
        /// need the key to find the contract anyway.
        #[arg(long)]
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
        /// Seeds the fixture set covers, 1..N. Stated, never defaulted: the differential
        /// downstream runs over exactly this many seeds, so a silent default would let a
        /// gate row emit a subset and still exit 0.
        #[arg(long, value_parser = seed_count())]
        seeds: u32,
        /// Output directory; `target/oracle-fixtures` by default.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Writes the stream fixtures the force gate's stream stage reads:
    /// `fixtures/stream-{small,hub,pow2}.jsonl`, one JSON v1 document per line, the first
    /// line the initial graph and every later line one batch.
    ///
    /// The same command generates the fixtures and the gate row that diffs them, so a
    /// fixture that drifted from the emitter is caught rather than read.
    EmitStreamFixtures {
        /// Output directory; the workspace's own `fixtures/` by default.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Runs `harness/oracle-diff.mjs` over the emitted fixtures (the TypeScript arm).
    OracleDiff {
        /// Fixtures directory; `target/oracle-fixtures` by default.
        ///
        /// **The default is RG-51's half that is still open**: a differential that falls
        /// back to whatever stale set is on disk reports it as this tree's verdict. Making
        /// it required is the fix, and it is blocked on `scripts/orch/rows/develop-full.rows`
        /// (`oracle-layouts`, which names no `--fixtures`) being updated by whoever owns the
        /// rows file — this crate's paths do not reach it. Tracked in
        /// `docs/measurements/fix-gates-hashgate.md`.
        #[arg(long)]
        fixtures: Option<PathBuf>,
    },
    /// The Python-armed differentials' own subcommands: `emit-<name>-fixtures` and
    /// `oracle-<name>`, one pair per differential (`oracle_python::cli`).
    #[command(flatten)]
    PythonOracle(crate::oracle_python::Cli),
    /// Runs `harness/oracle-layouts.mjs` over the emitted fixtures (the d3-hierarchy arm).
    OracleLayouts {
        /// Fixtures directory; `target/oracle-fixtures` by default — see
        /// [`Command::OracleDiff::fixtures`] for why the required form is pending.
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
        /// Number of seeds, 1..N.
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
    /// Node overlap removal: does the pass separate every pair, and what does it cost?
    /// Prints the overlapping-pair count before and after (an exhaustive `O(n^2)` check, so
    /// `--nodes` is capped), the mean displacement, and the stress ratio before and after.
    /// Exit 1 when any pair still overlaps — which is what `GM_MUTATE_OVERLAP_RELAXATION=0`
    /// makes it do.
    Overlap {
        /// A committed POST fixture, by name: `hairball`, `long-span`, `obstacles` or
        /// `parallel-edges`.
        #[arg(long, conflicts_with = "nodes", required_unless_present = "nodes")]
        fixture: Option<String>,
        /// The hairball generator's node count, any size. The exhaustive invariant scan is
        /// refused above 2 000 — it is `O(n^2)` — so a larger run needs `--no-scan`.
        #[arg(long)]
        nodes: Option<u32>,
        /// The layout that draws the graph first.
        #[arg(long, default_value = "layout.grid")]
        layout: String,
        /// The node radius the pass is asked to separate, in layout units. A layout emits
        /// `Point` centres, which have no extent, so without a radius there is nothing to
        /// separate and the pass is a documented no-op.
        #[arg(long, default_value_t = 1.0)]
        radius: f64,
        /// Skip the exhaustive `O(n^2)` invariant scan and read the pass's own grid count
        /// instead. Only for the scale sweep: the scan is refused above 2 000 nodes anyway,
        /// because it would cost more than the pass it checks.
        #[arg(long, default_value_t = false)]
        no_scan: bool,
        /// The pass's iteration cap, overriding `SeparateParams::max_iterations` (512).
        ///
        /// The escape hatch, and the only way to see what the cap costs: the default is sized
        /// from a lattice of at most 2 000 nodes, so a larger `--nodes` leaves a residue this
        /// flag can be raised against. `0` is refused, by name, like any other parameter.
        #[arg(long)]
        max_iterations: Option<u32>,
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
        /// defaulted, and vetted against that one word: a quality gate that silently
        /// picked its own baseline would be a gate comparing the implementation against
        /// itself, and a mistyped name would reach `stress` as an unknown oracle rather
        /// than as clap's error naming the possibility.
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(["d3"]))]
        oracle: String,
        /// The force layout measured, by registry id.
        #[arg(long, default_value = "layout.force.barnes_hut")]
        layout: String,
        /// Number of seeds, 1..N.
        #[arg(long, default_value_t = 8, value_parser = seed_count())]
        seeds: u32,
    },
    /// Wall time and Kruskal stress-1 of the Phase 6 layouts (or `--layout`) at the given
    /// node counts, refusing a size past a layout's own registered `scale_ceiling`.
    Bench(crate::bench::Plan),
    /// The wall time of single live-session ticks on the scale model, after a warm-up: the
    /// per-tick number the whole-stage `bench` averages away, and the profilers' workload.
    Tick(crate::bench::tick::Plan),
    /// One layout or post at one size: its wall time and peak resident memory as one
    /// `key=value` line, the rung of `scripts/caps-ladder.sh` (`docs/measurements/service-caps.md`).
    CapProbe(crate::bench::cap_probe::Plan),
    /// How far each many-body solver stands from the exact all-pairs sum, at the seed
    /// positions and after 100 Barnes-Hut ticks, at every `--n`. A `--require` solver must
    /// be no further from the exact sum than `bh:<the frozen theta>` is, on every set and
    /// every size — decision 2 of `docs/decisions/obsidian-force.md`. See
    /// `docs/measurements/perf-mb-fidelity.md`.
    MbFidelity(crate::mb_fidelity::Plan),
}

#[cfg(test)]
#[path = "command/tests.rs"]
mod tests;
