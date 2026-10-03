//! The table `mb-fidelity` prints, and the numbers in it.
//!
//! Markdown, because the run's output *is* the report: a table that has to be retyped
//! into `docs/measurements/perf-mb-fidelity.md` by hand is a table that gets edited.

use super::measure::{Row, SETTLE_TICKS};
use super::solver::Solver;

/// The report's markdown table: one row per `(size, set, solver)`, in run order.
///
/// `rms` is the number a pass is decided on — the relative RMS error against the exact
/// all-pairs sum — and `p50`/`p99` are the per-node relative error's median and 99th
/// percentile over the nodes whose exact force is non-zero. They are printed because a
/// small RMS can still hide one badly wrong node: a large p99 beside a small RMS is the
/// shape of a solver that is wrong in one region rather than everywhere.
pub fn table(rows: &[Row], sizes: &[u32]) -> String {
    let mut out = String::from(
        "| n | set | solver | rms | p50 | p99 | pass |\n\
         |---|---|---|---|---|---|---|\n",
    );
    for size in sizes {
        for row in rows.iter().filter(|r| r.n == *size) {
            out += &format!(
                "| {} | {} | {} | {} | {} | {} | {} |\n",
                row.n,
                row.set.label(),
                row.solver.label(),
                ratio(row.error.rms),
                ratio(row.error.p50),
                ratio(row.error.p99),
                verdict(row.pass),
            );
        }
    }
    out
}

/// The header a reader needs before the numbers mean anything: what the reference is, at
/// what tick the `settled` set was taken, what `pass` is measured against, and that a
/// non-zero coincidence count is printed rather than quietly averaged into the force.
pub fn preamble(sizes: &[u32], rows: &[Row]) -> String {
    let listed = sizes
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let baseline = Solver::Tree(super::baseline_theta()).label();
    format!(
        "sizes {listed} · `settled` = the positions after {SETTLE_TICKS} Barnes-Hut ticks on \
         the scale model\n\n\
         `pass` is `error.rms <= {baseline}`'s on the same set. `p50`/`p99` are the per-node \
         relative error's median and 99th percentile over the nodes whose exact force is \
         non-zero.\n\n\
         Self-check — `bh:0` opens every cell, so its rows are the exact sum in a different \
         order; each must be within `{tol:e}` relative RMS. A self-check that does not hold \
         means one of the two sums is wrong, so the run prints FAIL and exits 1 rather than \
         leaving a table nobody should read.\n\n\
         Coincident ordered pairs skipped by the exact sum, all sizes: {coincident}. A run \
         with a non-zero count is missing those terms from its own reference, and says so \
         here rather than reporting a short sum as the exact one.\n",
        tol = super::exact::SELF_CHECK_TOLERANCE,
        coincident = rows.iter().map(|r| r.coincident).sum::<u64>(),
    )
}

/// `yes`/`no`, not a tick and a cross: a row that passed has to read as passed to a script
/// reading the table as well as to a person reading it.
fn verdict(pass: bool) -> &'static str {
    if pass { "yes" } else { "no" }
}

/// One number as the table prints it: `%.3e`, and `nan` as itself rather than as a number
/// that looks measured.
fn ratio(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    format!("{value:.3e}")
}
