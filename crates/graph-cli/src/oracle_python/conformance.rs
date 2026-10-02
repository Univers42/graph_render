//! The SciGraphs byte-by-byte motor arm: the fixtures both sides read, the motor's own
//! coordinates over them, and the judge against the pinned baseline.
//!
//! Three steps, and the three are named for the differential like every other one here:
//!
//! 1. `graph-cli emit-conformance-fixtures --out DIR` — the graphs, the node-order mapping,
//!    and `motor/<NAME>.f64` + `.f32` per SciGraphs name;
//! 2. `harness/scigraphs-conformance.py` — `apply_graph_layout` itself for the names the
//!    oracle image can host, the Graphviz arm for the rest, then the metrics and one SVG per
//!    name;
//! 3. `graph-cli scigraphs-conformance --dir DIR` — the verdict, and
//!    `target/gates/scigraphs-conformance.json`.
//!
//! **Agreement here is bytes, not a tolerance.** `docs/measurements/scigraphs-coverage.md`
//! answers "within a tolerance"; this answers how close in bytes and what the first reason
//! is, per name, so a repair job can be written from the matrix instead of guessed at.
//!
//! Ponytail: the fixtures are emitted once and both arms read the same file, so the two
//! sides cannot disagree on what graph they were handed. What they can disagree on is the
//! answer, which is the only thing this measures.

mod baseline;
mod emit;
mod fixtures;
mod gaps;
mod motor;
mod rows;
mod verdict;

pub use rows::ROWS;

use std::path::Path;
use std::process::ExitCode;

/// Where a name's reference coordinates come from.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reference {
    /// `apply_graph_layout` itself, run by `harness/scigraphs-conformance.py` in
    /// `ge-python-oracle` with the `SciGraphs/` submodule on the path.
    Scigraphs,
    /// The `ge-graphviz-oracle` arm, because SciGraphs' own Graphviz path goes through
    /// `scigraphs_utils.graphviz_layout` (`yifan_hu.py:278-279`) and that package is not in
    /// the image. The engine named here is the one `GRAPHVIZ_ENGINES` maps the name to
    /// (`yifan_hu.py:7-16`).
    Graphviz(&'static str),
}

/// One parameter of `apply_graph_layout` the motor layout has no slot for.
///
/// Recorded, never normalised away: a row that quietly stood in `scale = 5.0` for a
/// hard-coded `SCALE = 5.0` would otherwise read as an arithmetic gap when the truth is that
/// the number cannot be changed.
pub struct Gap {
    /// The `apply_graph_layout` parameter: `iterations`, `scale` or the layout seed.
    pub parameter: &'static str,
    /// What the motor does instead, in one clause.
    pub note: &'static str,
    /// Where in the tree the fixed value lives.
    pub at: &'static str,
}

/// One row: a SciGraphs name and the motor layout compared against it.
pub struct Row {
    /// The `apply_graph_layout` name (`dispatcher.py:51-144`).
    pub name: &'static str,
    /// The motor layout id, or `None` where the tree has none.
    pub motor: Option<&'static str>,
    /// Which arm holds the reference for this name.
    pub reference: Reference,
    /// What the motor has no slot for.
    pub gaps: &'static [Gap],
}

impl Row {
    /// The `ge-graphviz-oracle` engine for this row's reference, `None` for the SciGraphs arm.
    pub fn engine(&self) -> Option<&'static str> {
        match self.reference {
            Reference::Scigraphs => None,
            Reference::Graphviz(engine) => Some(engine),
        }
    }

    /// The reference arm as the matrix and the ledger record it: `scigraphs` or
    /// `graphviz:<engine>`, so a reader never has to guess which image produced a row.
    pub fn reference_arm(&self) -> String {
        match self.engine() {
            Some(engine) => format!("graphviz:{engine}"),
            None => "scigraphs".to_string(),
        }
    }
}

/// `apply_graph_layout`'s own defaults (`dispatcher.py:14`), restated so the motor arm and
/// the reference arm are handed the same two numbers and a reader of this file does not have
/// to open the submodule to learn them.
pub const SCALE: f64 = 5.0;
pub const ITERATIONS: u32 = 50;

/// The layout seed `get_layout_seed()` returns with no pipeline seed set:
/// `derive_seed(42, "layout")` (`repro/determinism.py:124-129`, `:56-62`) is **981798123**.
/// Every motor row with a seed slot is given this value, so a row is never "a different RNG,
/// but at what seed?" — both sides are at this one.
pub const LAYOUT_SEED: u32 = 981_798_123;

/// `graph-cli emit-conformance-fixtures --out DIR`.
pub fn emit(out: &Path) -> ExitCode {
    match emit::write(out) {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("emit-conformance-fixtures: {err}");
            ExitCode::from(2)
        }
    }
}

/// `graph-cli scigraphs-conformance --dir DIR`.
pub fn judge(dir: &Path) -> ExitCode {
    match verdict::judge(dir) {
        Ok(pass) => {
            println!("{}", if pass { "PASS" } else { "FAIL" });
            ExitCode::from(u8::from(!pass))
        }
        Err(err) => {
            eprintln!("scigraphs-conformance: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests;
