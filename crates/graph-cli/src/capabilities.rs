//! The capabilities ledger (`prompt.md` §8): generated from the registry plus the last
//! recorded gate runs, never hand-written, so it cannot rot the way a PROGRESS.md does.
//!
//! `scale_ceiling`, `degradation` and `ponytail` are plain non-optional fields: a row
//! cannot be written without them, so no later phase can register a capability that
//! skips them. `hash_4way` and `oracle_diff` are not typed by anyone: they are derived
//! from `target/gates/*.json` (see `evidence.rs`), and a `gated` row stands only while
//! both verdicts hold for the tree as it is now. `--check` refuses every other claim.

mod analysis;
mod post;
mod registry;
mod verdict;

use serde::Serialize;
use std::collections::BTreeSet;
use std::process::ExitCode;
use verdict::Evidence;

/// Where a capability stands. Serialised lowercase: `absent | stub | implemented | gated`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the ledger's vocabulary (§8); every Phase-1 row is gated, lower statuses are for rows in progress"
    )
)]
pub enum Status {
    /// Not started.
    Absent,
    /// Present as a placeholder that does not compute the real thing.
    Stub,
    /// Computes the real thing; its gate evidence is not (yet) complete.
    Implemented,
    /// Its 4-way hash and its oracle differential both passed in the current tree.
    Gated,
}

/// One ledger row. Field names are the JSON keys of `prompt.md` §8.
#[derive(Debug, Clone, Serialize)]
pub struct Capability {
    /// Stable dotted id, e.g. `layout.treemap.squarified`.
    pub id: &'static str,
    /// Delivery tier.
    pub tier: u8,
    /// Pipeline stage: ingest, topology, analysis, layout, post, scale.
    pub stage: &'static str,
    /// Geometry kind it emits, for stages that emit geometry.
    pub geometry: Option<&'static str>,
    /// Where it claims to stand; `--check` refuses a claim its evidence does not back.
    pub status: Status,
    /// The reference it is differentially tested against.
    pub oracle: &'static str,
    /// The gate record its oracle verdict is read from: `oracle-diff`, `roundtrip` or
    /// `oracle-layouts`.
    #[serde(skip)]
    pub oracle_record: &'static str,
    /// The oracle functions whose differential backs it.
    #[serde(skip)]
    pub functions: &'static [&'static str],
    /// The hash-gate stage that covers it.
    #[serde(skip)]
    pub hash_stage: &'static str,
    /// Outcome of that differential, from the recorded run.
    pub oracle_diff: String,
    /// Outcome of the 4-way hash gate, from the recorded run.
    pub hash_4way: String,
    /// Node count past which it stops being usable. Required.
    pub scale_ceiling: u64,
    /// What happens past the ceiling. Required.
    pub degradation: &'static str,
    /// Its Ponytail marker, or the reason none is owed. Required.
    pub ponytail: &'static str,
    /// Time complexity, stated and held.
    pub complexity: &'static str,
}

/// Every registered capability: Phase 1/2's rows, unchanged, plus Phase 7's `analysis`
/// rows (`analysis.rs`). `registry::registry()` itself is untouched — this only chains
/// onto it, so the phase's authorization envelope (`capabilities.rs`, not
/// `capabilities/registry.rs`) is what actually changed.
pub fn registry() -> Vec<Capability> {
    let mut rows = registry::registry();
    rows.extend(analysis::rows());
    rows
}

/// The rows with `oracle_diff` and `hash_4way` filled from `evidence`: the verdict, or
/// `not backed: <why>`.
pub fn ledger(evidence: &Evidence) -> Vec<Capability> {
    let text = |verdict: Result<String, String>| {
        verdict.unwrap_or_else(|why| format!("not backed: {why}"))
    };
    let mut rows = registry();
    for row in &mut rows {
        row.oracle_diff = text(verdict::oracle_diff(
            evidence,
            row.oracle_record,
            row.functions,
        ));
        row.hash_4way = text(verdict::hash_4way(evidence, row.hash_stage));
    }
    rows
}

/// Every reason a row may not stand as written; empty means the ledger is honest.
pub fn problems(rows: &[Capability], evidence: &Evidence) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut found = Vec::new();
    for row in rows {
        if !seen.insert(row.id) {
            found.push(format!("{}: duplicate id", row.id));
        }
        for (field, value) in [
            ("stage", row.stage),
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
            ("complexity", row.complexity),
        ] {
            if value.trim().is_empty() {
                found.push(format!("{}: required field `{field}` is empty", row.id));
            }
        }
        if row.scale_ceiling == 0 {
            found.push(format!("{}: scale_ceiling is 0", row.id));
        }
        if row.status == Status::Gated {
            let verdicts = [
                verdict::hash_4way(evidence, row.hash_stage),
                verdict::oracle_diff(evidence, row.oracle_record, row.functions),
            ];
            for why in verdicts.into_iter().filter_map(Result::err) {
                found.push(format!("{}: gated, but {why}", row.id));
            }
        }
    }
    found
}

/// `capabilities --json` and/or `--check`.
pub fn run(json: bool, check: bool) -> ExitCode {
    if !json && !check {
        eprintln!("capabilities: pass --json, --check, or both");
        return ExitCode::from(2);
    }
    let evidence = match Evidence::load() {
        Ok(evidence) => evidence,
        Err(err) => {
            eprintln!("capabilities: reading the gate records: {err}");
            return ExitCode::from(2);
        }
    };
    let rows = ledger(&evidence);
    if json {
        match serde_json::to_string_pretty(&rows) {
            Ok(text) => println!("{text}"),
            Err(err) => {
                eprintln!("capabilities: serialising the ledger: {err}");
                return ExitCode::from(2);
            }
        }
    }
    if check {
        let found = problems(&rows, &evidence);
        for problem in &found {
            println!("  {problem}");
        }
        println!(
            "capabilities --check: {} rows, {} problems",
            rows.len(),
            found.len()
        );
        if !found.is_empty() {
            return ExitCode::from(1);
        }
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests;
