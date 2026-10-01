//! `graph-cli oracle-spring`: the verdict half of the spring differential, kept out of
//! `../spring.rs` by the house line cap and split from its measuring half
//! (`measure.rs`) and its tests (`tests.rs`) by the same cap. This file holds what decides:
//! the ceiling, the line it prints, and the record it writes. `measure.rs` holds the two
//! arms, the metric and the exact assertions.
//!
//! **The metric is computed once, in `measure.rs`,** and it is graph-core's own
//! [`crate::stress::metric::correlate`] — the Pearson hop/euclidean function `graph-cli
//! stress` and the d3 arm use. The harness writes the reference's positions and computes
//! nothing (`harness/oracle-spring.py`, which says why: a metric restated in the arm as
//! well would be two implementations whose disagreement would be indistinguishable from a
//! disagreement between two layouts). The deficit is `max(0, theirs - ours)`.
//!
//! Ponytail: the verdict is on the **median** deficit and the worst, the 90th percentile
//! and the worst case strictly below 500 are printed and recorded beside it, because the
//! extreme of 1000 small-graph samples is tail noise and a reader is owed the shape and not
//! only the gated number. The `n >= 500` fork — where networkx's `method="auto"`
//! (`layout.py:140-141`) hands the graph to an L-BFGS energy minimiser this port does not
//! reproduce — is inside the compared set, so the worst case strictly below 500 is printed
//! too: the ceiling can then be read either way.

mod measure;
#[cfg(test)]
mod tests;

use super::super::read;
use super::SPRING;
use crate::evidence::Stamp;
use serde_json::{Value, json};
use std::path::Path;
use std::process::ExitCode;

/// The whole measured run: enough to print the shape of the deficit and to judge it.
struct Summary {
    cases: usize,
    scored: usize,
    degenerate: usize,
    worst: f64,
    worst_seed: u32,
    worst_below_500: f64,
    median: f64,
    p90: f64,
}

/// `graph-cli oracle-spring --dir <dir>`: reads both arms, applies the metric to both,
/// records the verdict under `oracle-spring`. Exit 0 pass · 1 a case over the ceiling ·
/// 2 could not run.
pub fn ingest(dir: &Path) -> ExitCode {
    match verdict(dir) {
        Ok(pass) => ExitCode::from(u8::from(!pass)),
        Err(err) => {
            eprintln!("oracle-spring: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

fn verdict(dir: &Path) -> Result<bool, String> {
    let stamp = Stamp::take()?;
    let manifest = read(&dir.join("spring-manifest.json"))?;
    check_tree(&manifest, &stamp, dir)?;
    let sum = measure::run(dir, &manifest)?;
    if sum.cases == 0 || sum.scored == 0 {
        return Err("a differential over nothing proves nothing".into());
    }
    let (id, _, ceiling) = SPRING.ceilings[0];
    let within = sum.median <= ceiling;
    println!(
        "  {id}: {}/{} cases correlated ({} no correlation exists), median deficit \
     {:.3e} at ceiling {:.0e}: {} [worst {:.3e} at seed {}, p90 {:.3e}, worst below \
     500 {:.3e}]",
        sum.scored,
        sum.cases,
        sum.degenerate,
        sum.median,
        ceiling,
        if within { "ok" } else { "FAIL" },
        sum.worst,
        sum.worst_seed,
        sum.p90,
        sum.worst_below_500
    );
    stamp.still_current()?;
    crate::evidence::record(&stamp, "oracle-spring", body(&sum))?;
    println!("{}", if within { "PASS" } else { "FAIL" });
    Ok(within)
}

/// **The fixtures, the arm and the tree must be the same tree**, exactly as
/// `oracle_python`'s other differentials insist (`verdict`, at
/// `crates/graph-cli/src/oracle_python.rs:136-143`). This arm reads two files the manifest
/// fingerprinted only one of, so it checks both: the fixtures against the manifest's digest,
/// and the manifest against the tree's own stamp. Without the first a stale harness run
/// would be measured against re-emitted fixtures; without the second a whole tree's gate
/// records would be written by fixtures of an older tree.
fn check_tree(manifest: &Value, stamp: &Stamp, dir: &Path) -> Result<(), String> {
    let fixtures = dir.join("spring.jsonl");
    let digest = crate::runner::file_sha256(&fixtures)?;
    if manifest["sha256"]["spring.jsonl"] != digest {
        return Err(format!(
            "{} does not match its manifest: re-emit and re-run the arm",
            fixtures.display()
        ));
    }
    if manifest["fingerprint"] != stamp.fingerprint() {
        return Err("fixtures and tree are not the same tree: re-emit and re-run".into());
    }
    Ok(())
}

/// The recorded verdict, in the shape `oracle_python`'s other differentials write.
fn body(sum: &Summary) -> Value {
    let (id, _, ceiling) = SPRING.ceilings[0];
    json!({
        "seeds": sum.cases,
        "pass": sum.median <= ceiling,
        "tolerance": true,
        "metric": "Pearson hop/euclidean deficit max(0, theirs - ours) over 32 max-min \
                   pivots, graph-core's own crates/graph-cli/src/stress/metric.rs applied to \
                   both arms; the ceiling is on the MEDIAN over seeds, the worst and the \
                   90th percentile are recorded beside it and are not gated",
        "oracle": "networkx 3.6 spring_layout(dim=2), the arm harness/oracle-spring.py runs",
        "functions": {
            id: {
                "cases": sum.scored,
                "declared": sum.degenerate,
                "unexplained": u64::from(sum.median > ceiling),
                "median": sum.median, "ceiling": ceiling,
                "worst": sum.worst, "worst_seed": sum.worst_seed,
                "p90": sum.p90, "worst_below_500": sum.worst_below_500,
            },
        },
    })
}

impl Default for Summary {
    fn default() -> Self {
        Summary {
            cases: 0,
            scored: 0,
            degenerate: 0,
            worst: 0.0,
            worst_seed: 0,
            worst_below_500: 0.0,
            median: 0.0,
            p90: 0.0,
        }
    }
}
