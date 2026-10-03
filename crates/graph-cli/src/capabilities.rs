//! The capabilities ledger (`prompt.md` §8): generated from the registry plus the last
//! recorded gate runs, never hand-written, so it cannot rot the way a PROGRESS.md does.
//!
//! `scale_ceiling`, `degradation` and `ponytail` are plain non-optional fields: a row
//! cannot be written without them, so no later phase can register a capability that
//! skips them. `hash_4way` and `oracle_diff` are not typed by anyone: they are derived
//! from `target/gates/*.json` (see `evidence.rs`), and a `gated` row stands only while
//! both verdicts hold for the tree as it is now. `--check` refuses every other claim.

mod analysis;
mod ceilings;
mod ingest;
mod post;
mod registry;
mod scale;
mod verdict;

use crate::runner::workspace_root;
use ceilings::{ceiling_findings, read_ceilings_doc_at};
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

impl Status {
    /// The word this status serialises as and prints as: one spelling, not two.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Stub => "stub",
            Self::Implemented => "implemented",
            Self::Gated => "gated",
        }
    }
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

/// Every registered capability, in one list built by one call.
///
/// The registry itself is untouched — `registry::registry()` chains every row family onto
/// graph-core's own registries, and this only names that one entry point, so the phase's
/// authorization envelope (`capabilities.rs`, not `capabilities/registry.rs`) is what
/// changed.
pub fn registry() -> Vec<Capability> {
    registry::registry()
}

/// Where Phase 9's ceiling measurements are read from, and what the row's own numbers
/// are read against.
pub const CEILINGS_DOC: &str = "docs/measurements/phase09-ceilings.md";

/// The highest `scale_ceiling` a row may declare, and why this one.
///
/// A ceiling is a node count past which the capability stops being usable, and the
/// largest one this tree declares is the topology layer's `TOPOLOGY_CEILING`
/// (9 700 000, `registry.rs`). Nothing above it is a measurement: `u64::MAX` was
/// accepted before this bound existed, and every `n <= ceiling` test downstream of it is
/// then vacuously true — a ceiling that refuses nothing is a ceiling that says nothing.
/// Every declared ceiling is a named constant in graph-core or above, so this bound
/// refuses only a literal nobody could mean.
pub const MAX_SCALE_CEILING: u64 = registry::TOPOLOGY_CEILING;

/// The prefix [`ledger`] puts in front of a verdict that no recorded run backs. A cell
/// that does not start with it is a verdict.
pub(super) const NO_BACKED: &str = "not backed: ";

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
        // `oracle` is in the sweep for the same reason `complexity` is: a row that names
        // no reference and checks no function (see `verdict::oracle_diff`) is a row that
        // claims a differential it does not have.
        for (field, value) in [
            ("stage", row.stage),
            ("oracle", row.oracle),
            ("degradation", row.degradation),
            ("ponytail", row.ponytail),
            ("complexity", row.complexity),
        ] {
            if value.trim().is_empty() {
                found.push(format!("{}: required field `{field}` is empty", row.id));
            }
        }
        found.extend(ceiling_problems(row));
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

/// What one row's own numbers say about it, with no reference to any evidence: a ceiling
/// of 0, a ceiling above [`MAX_SCALE_CEILING`], and a geometry kind this ledger has no
/// name for.
fn ceiling_problems(row: &Capability) -> Vec<String> {
    let mut found = Vec::new();
    if row.scale_ceiling == 0 {
        found.push(format!("{}: scale_ceiling is 0", row.id));
    }
    if row.scale_ceiling > MAX_SCALE_CEILING {
        found.push(format!(
            "{}: scale_ceiling {} is above the largest one this tree declares ({MAX_SCALE_CEILING})",
            row.id, row.scale_ceiling
        ));
    }
    found
}

/// One line per row under `--check`: the status it claims and whether a recorded run backs
/// it. A green `--check` used to say nothing about the difference between a row with
/// evidence and a row with none, and more than half this ledger ships with none — so
/// "0 problems" read the same either way.
fn print_verdicts(rows: &[Capability]) {
    for row in rows {
        println!(
            "  {:<11} {:<42} {}",
            row.status.as_str(),
            row.id,
            how_backed(row)
        );
    }
}

fn how_backed(row: &Capability) -> &'static str {
    match (
        row.oracle_diff.starts_with(NO_BACKED),
        row.hash_4way.starts_with(NO_BACKED),
    ) {
        (false, false) => "backed by a recorded run on this tree",
        (true, true) => "no evidence: nothing recorded backs it",
        _ => "partly backed: one verdict, not both",
    }
}

/// Every reason the ledger is not honest, as `--check` reports it: the rows' own
/// problems, and the ceilings table read from `ceilings_doc`.
///
/// The ceilings table is read by `--check` and not only by the flag that names it: a
/// missing doc, a row the ledger does not have, or a measured cell that contradicts the
/// ceiling the row declares is a finding about the ledger, and it used to be invisible
/// unless `--ceilings-measured` was passed as well. The path is a parameter so the test
/// can point it at a doc the tree does not have.
fn check_findings(
    rows: &[Capability],
    evidence: &Evidence,
    ceilings_doc: &std::path::Path,
) -> Vec<String> {
    let mut found = problems(rows, evidence);
    found.extend(read_ceilings_doc_at(ceilings_doc).map_or_else(
        |err| vec![format!("{CEILINGS_DOC}: {err}")],
        |doc| ceiling_findings(rows, &doc),
    ));
    found
}

/// `capabilities --json`, `--check` and/or `--ceilings-measured`.
pub fn run(json: bool, check: bool, ceilings_measured: bool) -> ExitCode {
    if !json && !check && !ceilings_measured {
        eprintln!("capabilities: pass --json, --check, --ceilings-measured, or more than one");
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
        let found = check_findings(&rows, &evidence, &workspace_root().join(CEILINGS_DOC));
        print_verdicts(&rows);
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
    if ceilings_measured && !ceilings::check_ceilings(&rows) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests;
