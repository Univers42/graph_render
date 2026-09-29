//! The capabilities ledger (`prompt.md` §8): generated from the registry plus the last
//! recorded gate runs, never hand-written, so it cannot rot the way a PROGRESS.md does.
//!
//! `scale_ceiling`, `degradation` and `ponytail` are plain non-optional fields: a row
//! cannot be written without them, so no later phase can register a capability that
//! skips them. `hash_4way` and `oracle_diff` are not typed by anyone: they are derived
//! from `target/gates/*.json` (see `evidence.rs`), and a `gated` row stands only while
//! both verdicts hold for the tree as it is now. `--check` refuses every other claim.

mod analysis;
mod ingest;
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

/// Every registered capability: Phase 1/2's rows, unchanged, Phase 7's `analysis` rows
/// (`analysis.rs`), Phase 9's three `scale` rows ([`scale_rows`]) and Phase 10's `ingest` rows (`ingest.rs`). The layout and
/// analysis registries themselves are untouched — this only chains onto them, so the
/// phase's authorization envelope (`capabilities.rs`, not `capabilities/registry.rs`) is
/// what actually changed.
pub fn registry() -> Vec<Capability> {
    let mut rows = registry::registry();
    rows.extend(analysis::rows());
    rows.extend(scale_rows());
    rows.extend(ingest::rows());
    rows
}

/// Where Phase 9's ceiling measurements are read from, and what the row's own numbers
/// are read against.
pub const CEILINGS_DOC: &str = "docs/measurements/phase09-ceilings.md";

/// Phase 9's three `scale` rows.
///
/// `Status::Implemented`, never `Gated`: `graph-core`'s scale stage is not in the hash
/// gate's stage list (that file is outside this phase's envelope) and has no oracle
/// differential, so `gated` would be a claim `problems()` refuses for the right reason.
/// Promoting them is the merge step's work, recorded in `docs/reports/phase-09-progress.md`.
///
/// `scale_ceiling` here is **inherited and reasoned, not measured**: these are `O(n)` and
/// `O(n + m)` passes over a topology that is already indexed, so they are bounded by
/// topology's own measured per-node cost, exactly as `analysis`'s rows are. The measured
/// ceiling campaign is [`CEILINGS_DOC`]'s subject.
fn scale_rows() -> Vec<Capability> {
    use registry::TOPOLOGY_CEILING;
    let ceiling = TOPOLOGY_CEILING;
    let common = |oracle: &'static str,
                  complexity: &'static str,
                  degradation: &'static str,
                  ponytail: &'static str| Capability {
        id: "",
        tier: 1,
        stage: "scale",
        geometry: None,
        status: Status::Implemented,
        oracle,
        oracle_record: "oracle-diff",
        functions: &[],
        hash_stage: "topology",
        oracle_diff: String::new(),
        hash_4way: String::new(),
        scale_ceiling: ceiling,
        degradation,
        ponytail,
        complexity,
    };
    vec![
        Capability {
            id: "scale.lod",
            oracle: "SciGraphs engine/scigraphs_engine/lod.py: the budget rule and the never-empty mask, ported; the pixel thresholds are not (no camera here) and the tier ladder is the phase's own",
            complexity: "O(n + m), one pass each, no spatial structure",
            degradation: "advisory by construction: the hints are columns a front may ignore entirely, and the topology is never mutated, so past the ceiling the only cost is a front that chose to draw everything",
            ponytail: "Ponytail: the thresholds are a heuristic and it fails in the dangerous direction. Failing input: a graph whose important nodes are low-degree (a dependency graph's entry points, a star's hub the budget ranks low), where a degree-ranked label budget hides exactly what a reader came for. Direction: hiding meaningful nodes. Escape hatch: ignore the hints; they are advisory. Second heuristic, same shape: edge decimation is a stride over edge index, so a graph whose long-range edges share one stride class loses all of them",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.simplify",
            oracle: "hand: degree-1 folding, maximal degree-2 chain walks, and Phase 7's analysis.communities (louvain) for the collapse. The reference's simplify.py extracts a backbone (MST/disparity/top-k) and coarsens for bundling; neither is a reversible reduction of the graph, so neither is ported",
            complexity: "O(n + m log m) to build the simple adjacency, then O(n + m) per pass",
            degradation: "past the ceiling, the same shape as topology: wasm32 cannot allocate and the module traps; natively, memory permitting, this refuses alongside index_model's own CapacityError. Every removal is journalled, so a front that ignored the ceiling would still be able to restore",
            ponytail: "Ponytail: the community collapse trusts louvain, a heuristic. Failing input: near-tied modularity gains, or a graph whose communities are single-edge chains, where a collapse removes the node a reader came to see. Direction: cosmetic, because the journal still holds it — the dangerous version, an irreversible collapse, is not implemented. Escape hatch: Plan::collapse_communities off",
            ..common("", "", "", "")
        },
        Capability {
            id: "scale.adaptive",
            oracle: "SciGraphs engine/scigraphs_engine/adaptive.py: the same intent (bounded work per settle), deliberately NOT its mechanism — it adapts from measured crowding and a camera at render time, which here would mean reading a clock, which D8 forbids inside the motor",
            complexity: "O(1): a pure function of (n, m)",
            degradation: "none: the budget never fails and never allocates. What degrades is the layout's settle at large n, which is the trade the row exists to make and the caller's iteration override takes back",
            ponytail: "Ponytail: a large graph gets fewer ticks and a less settled layout. Failing input: any graph past ~3 000 nodes, whose tails are still moving when the budget runs out. Direction: cosmetic. Escape hatch: tick_budget_with's explicit override, honoured verbatim including 0",
            ..common("", "", "", "")
        },
    ]
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

/// The `measured` cell of every row of the ceilings table, by id: the table's own first
/// column is the id, its third is what Phase 9 measured (`not measured` for a row it
/// declares but has not run).
fn ceiling_table(doc: &str) -> Vec<(String, String)> {
    doc.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let id = cells
                .get(1)
                .filter(|c| !c.is_empty() && **c != "id" && !c.starts_with('-'))?;
            let measured = cells.get(3)?;
            Some(((*id).to_owned(), (*measured).to_owned()))
        })
        .collect()
}

/// Every reason the ceilings table is not an honest record of what was measured: a row
/// the ledger does not have, and a row whose measured cell is not a number.
///
/// A ledger row the table says nothing about is deliberately **not** a finding: this
/// phase measured some ceilings and not others, and the count [`ceiling_coverage`]
/// prints is where the ones it did not are named. Turning "not yet measured" into a
/// silent pass would be the dishonest shape; turning it into a hard failure for the
/// twenty-odd layouts this phase never ran would make the flag unusable.
pub fn ceiling_findings(rows: &[Capability], doc: &str) -> Vec<String> {
    let table = ceiling_table(doc);
    let mut found: Vec<String> = Vec::new();
    for (id, measured) in &table {
        let Some(row) = rows.iter().find(|row| row.id == id) else {
            found.push(format!(
                "{id}: the table names a row the ledger does not have"
            ));
            continue;
        };
        if !is_measured(measured) {
            found.push(format!(
                "{id}: the table's measured cell is `{measured}`, not a number (declared {})",
                row.scale_ceiling
            ));
        }
    }
    found
}

/// A measured cell: a run of digits and nothing else. `not measured` is a cell this
/// function refuses on purpose — it is how the table says "declared, not run".
fn is_measured(cell: &str) -> bool {
    !cell.is_empty() && cell.chars().all(|c| c.is_ascii_digit())
}

/// How many ledger rows the table measures, and how many it leaves reasoned.
pub fn ceiling_coverage(rows: &[Capability], doc: &str) -> (usize, usize) {
    let table = ceiling_table(doc);
    let measured = rows
        .iter()
        .filter(|row| {
            table
                .iter()
                .any(|(id, value)| id == row.id && is_measured(value))
        })
        .count();
    (measured, rows.len() - measured)
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
    if ceilings_measured && !check_ceilings(&rows) {
        return ExitCode::from(1);
    }
    ExitCode::SUCCESS
}

/// `--ceilings-measured`: reads [`CEILINGS_DOC`] and reports what it measures.
fn check_ceilings(rows: &[Capability]) -> bool {
    let path = crate::runner::workspace_root().join(CEILINGS_DOC);
    let doc = match std::fs::read_to_string(&path) {
        Ok(doc) => doc,
        Err(err) => {
            println!("  {CEILINGS_DOC}: {err}");
            return false;
        }
    };
    let found = ceiling_findings(rows, &doc);
    let (measured, reasoned) = ceiling_coverage(rows, &doc);
    for problem in &found {
        println!("  {problem}");
    }
    println!(
        "ceilings measured: {measured} of {} rows; {reasoned} still reasoned",
        rows.len()
    );
    found.is_empty()
}

#[cfg(test)]
mod tests;
