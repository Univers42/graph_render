//! `graph-cli mb-fidelity`: how far each many-body solver stands from the exact all-pairs
//! sum, at two position sets and several sizes.
//!
//! # What this decides
//!
//! `docs/decisions/obsidian-force.md` decision 2 lets a many-body solver stand in for
//! Barnes-Hut at `theta = 0.9` only if its force error against the exact sum is no larger
//! than Barnes-Hut's own error on the same positions. This is the instrument that measures
//! it, and it measures **forces at fixed positions** — one pass, `alpha` 1, no tick around
//! it. It does not measure a layout's output, and `docs/measurements/perf-mb-fidelity.md`
//! says so where the numbers are.
//!
//! # The three numbers
//!
//! * `rms` — `sqrt(Σ|F − F*|²) / sqrt(Σ|F*|²)`, the relative RMS error. This is what a
//!   pass is decided on.
//! * `p50`, `p99` — the per-node relative error's median and 99th percentile, over the
//!   nodes whose exact force is non-zero. Printed because a small RMS can hide one badly
//!   wrong node.
//! * `coincident` — ordered pairs the exact sum skipped. A non-zero count is printed and
//!   named, because a reference short of terms is not the exact sum it claims to be.
//!
//! # The self-check
//!
//! Before any row is trusted, Barnes-Hut at `bh:0` is measured against the same reference.
//! At `theta = 0` the opening threshold is infinite, so the walk descends to every leaf and
//! sums every pair — the exact sum in a different order, and therefore equal to it to
//! rounding. A self-check above [`exact::SELF_CHECK_TOLERANCE`] means one of the two sums is
//! wrong, and the run says so and exits **1** — it ran, and it failed — rather than letting
//! a table nobody should read go out as a result.
//!
//! # Exit codes
//!
//! `0` ran, the self-check held, and every `--require` solver passed on every set ·
//! `1` ran and either the self-check or a required solver failed · `2` could not run.

pub mod exact;
pub mod measure;
pub mod report;
pub mod solver;

use clap::builder::TypedValueParser;

use self::measure::Row;
use self::solver::Solver;
use graph_core::Topology;
use graph_core::layout::force::ForceParams;
use serde_json::json;
use std::process::ExitCode;

/// The gate's own record name, the way `oracle_python.rs` names its oracles.
pub const RECORD: &str = "mb-fidelity";

/// The Barnes-Hut angle every other solver is read against: the frozen default, read from
/// the parameter set rather than typed here, so a change to the default moves the bar and
/// the bar's own label together.
pub fn baseline_theta() -> f64 {
    ForceParams::default().theta
}

/// What one `graph-cli mb-fidelity` invocation measures.
#[derive(clap::Args)]
pub struct Plan {
    /// Node counts, comma separated: `1000,10000,50000`.
    ///
    /// 2..=`MAX_SCALE_NODES`, the same bound `bench --n` takes, so a typo is a clap error
    /// naming the range rather than a run that allocates the exact sum's `n²` for a
    /// nonsense size.
    #[arg(long, default_value = "1000,10000,50000", value_delimiter = ',', value_parser = size())]
    pub n: Vec<u32>,
    /// A solver that must be no further from the exact sum than `bh:<baseline theta>` is,
    /// on every set and every size. Repeatable; `pm` and `bh:<theta>` are the words.
    ///
    /// `bh:0` is accepted and always passes its own self-check, so naming it is a way of
    /// asking for the self-check alone.
    #[arg(long = "require", value_parser = parse_solver())]
    pub require: Vec<Solver>,
}

/// `--n`'s value parser: 2..=`MAX_SCALE_NODES`, the same bound `bench --n` takes. Two at the
/// bottom because a one-node graph's many-body sum is empty and the RMS against it is a
/// division by zero, which would be a `nan` in the table rather than an error.
fn size() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(2..=i64::from(crate::bench::scale::MAX_SCALE_NODES))
}

/// `--require`'s value parser: `Solver::from_str` through clap, so the accepted words are one
/// list and the argument error is clap's.
fn parse_solver() -> impl clap::builder::TypedValueParser {
    clap::builder::StringValueParser::new().try_map(|word| word.parse::<Solver>())
}

pub fn run(plan: &Plan) -> ExitCode {
    let stamp = match crate::evidence::Stamp::take() {
        Ok(stamp) => stamp,
        Err(err) => return could_not_run(&err),
    };
    match measure_all(&plan.n, &plan.require) {
        Ok(report) => finish(&stamp, &report, &plan.require),
        Err(err) => could_not_run(&err),
    }
}

/// Every size's rows, the sizes, and whether the self-check held everywhere.
struct Report {
    rows: Vec<Row>,
    sizes: Vec<u32>,
    self_check_held: bool,
}

/// Runs every size and collects the rows.
///
/// The self-check runs at every size and set, not once: a reference that agrees with the
/// tree at 1000 nodes and not at 50000 is a reference whose agreement was a coincidence of
/// size, and the number that says so is the one this job's report has to carry.
fn measure_all(sizes: &[u32], require: &[Solver]) -> Result<Report, String> {
    let solvers = solvers_to_run(require);
    let mut rows = Vec::new();
    let mut held = true;
    for size in sizes {
        let topology = topology_at(*size)?;
        let measured = measure::measure(&topology, *size, &solvers);
        held &= self_check_held(&measured);
        rows.extend(measured);
    }
    Ok(Report {
        rows,
        sizes: sizes.to_vec(),
        self_check_held: held,
    })
}

/// The scale model's topology at `n` nodes, the same model `bench --n` builds.
fn topology_at(n: u32) -> Result<Topology, String> {
    let (nodes, edges) = crate::bench::scale::scale_model(0, n, graph_core::REFERENCE_DEGREE);
    graph_core::index_model(&nodes, &edges).map_err(|err| format!("indexing {n} nodes: {err}"))
}

/// The solvers every run measures, in report order: the self-check, the baseline, and then
/// the mesh and the caller's `--require` words, deduplicated and in that fixed order so two
/// runs print the same table for the same sizes.
fn solvers_to_run(require: &[Solver]) -> Vec<Solver> {
    let mut solvers = vec![
        Solver::Tree(0.0),
        Solver::Tree(baseline_theta()),
        Solver::Mesh,
    ];
    for solver in require {
        if !solvers.contains(solver) {
            solvers.push(*solver);
        }
    }
    solvers
}

/// Whether every `bh:0` row — the self-check — came in at or under the tolerance.
fn self_check_held(rows: &[Row]) -> bool {
    rows.iter()
        .filter(|row| row.solver == Solver::Tree(0.0))
        .all(|row| row.error.rms <= exact::SELF_CHECK_TOLERANCE)
}

/// Prints the report, records it, and turns the two verdicts into an exit code.
fn finish(stamp: &crate::evidence::Stamp, report: &Report, require: &[Solver]) -> ExitCode {
    print!("{}", report::preamble(&report.sizes, &report.rows));
    print!("{}", report::table(&report.rows, &report.sizes));

    let required_pass = required_rows(&report.rows, require)
        .iter()
        .all(|row| row.pass);
    let pass = report.self_check_held && required_pass;

    let body = json!({
        "sizes": report.sizes,
        "reference": "O(n^2) all-pairs many-body sum, d3-force@3.0.0 manyBody.js's per-pair \
                      rule line for line, at alpha 1, coincident pairs skipped and counted",
        "baseline": Solver::Tree(baseline_theta()).label(),
        "metric": "relative RMS error sqrt(sum |F-F*|^2)/sqrt(sum |F*|^2) against an O(n^2) \
                   all-pairs sum at alpha 1; p50/p99 are the per-node relative error's \
                   median and 99th percentile over nodes with |F*| > 0",
        "settle_ticks": measure::SETTLE_TICKS,
        "self_check_tolerance": exact::SELF_CHECK_TOLERANCE,
        "self_check_held": report.self_check_held,
        "coincident": report.rows.iter().map(|row| row.coincident).sum::<u64>(),
        "require": require.iter().map(|s| s.label()).collect::<Vec<_>>(),
        "required_pass": required_pass,
        "rows": report.rows.iter().map(row_json).collect::<Vec<_>>(),
        "pass": pass,
    });
    if let Err(err) = crate::evidence::record(stamp, RECORD, body) {
        eprintln!("mb-fidelity: not recorded: {err}");
        return ExitCode::from(2);
    }
    if !report.self_check_held {
        println!(
            "FAIL: the bh:0 self-check did not hold within {:e}; one of the two sums is wrong",
            exact::SELF_CHECK_TOLERANCE
        );
        return ExitCode::from(1);
    }
    let missed: Vec<String> = required_rows(&report.rows, require)
        .iter()
        .filter(|row| !row.pass)
        .map(|row| {
            format!(
                "n={} {} {}: rms {:.3e} against {}'s {:.3e}",
                row.n,
                row.set.label(),
                row.solver.label(),
                row.error.rms,
                Solver::Tree(baseline_theta()).label(),
                row.baseline_rms,
            )
        })
        .collect();
    if missed.is_empty() {
        println!("PASS");
        return ExitCode::SUCCESS;
    }
    println!("FAIL: {}", missed.join("; "));
    ExitCode::from(1)
}

/// The rows a `--require` word is judged on: every size, every set, that solver.
fn required_rows<'a>(rows: &'a [Row], require: &[Solver]) -> Vec<&'a Row> {
    rows.iter()
        .filter(|row| require.contains(&row.solver))
        .collect()
}

/// One row as the record holds it: the numbers, and the pass that was decided on.
fn row_json(row: &Row) -> serde_json::Value {
    json!({
        "n": row.n,
        "set": row.set.label(),
        "solver": row.solver.label(),
        "rms": row.error.rms,
        "p50": row.error.p50,
        "p99": row.error.p99,
        "scored_nodes": row.error.scored,
        "coincident": row.coincident,
        "baseline_rms": row.baseline_rms,
        "pass": row.pass,
    })
}

/// Exit 2: the run could not happen, and its own reason is the only thing worth printing.
fn could_not_run(err: &str) -> ExitCode {
    eprintln!("mb-fidelity: could not run: {err}");
    ExitCode::from(2)
}
