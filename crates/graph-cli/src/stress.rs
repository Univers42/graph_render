//! `graph-cli stress --oracle d3`: the quality gate a force layout is actually held to.
//!
//! Phase 3's layouts have oracles that agree *exactly*. Force layouts do not, and
//! cannot: a force simulation amplifies a 1-ULP difference into a visibly different
//! picture within a couple of hundred ticks. There is no "close enough", and a
//! tolerance would not mean anything if there were. The honest claim is **"different,
//! but not worse"**, and this command is what makes the "not worse" falsifiable.
//!
//! Three checks, and the order matters:
//!
//! 1. **Determinism** — the 4-way hash gate, [`crate::hashgate`], which is
//!    non-negotiable and already covers both force stages.
//! 2. **Quality, not identity** — this command. Our stress correlation must clear
//!    `d3-force@3.0.0`'s own by [`MARGIN`], per case and on the median.
//! 3. **A measured speed win** — [`crate::bench`], at N = 220 / 10 000 / 100 000.
//!
//! The d3 arm is a real `d3-force@3.0.0` simulation run by `harness/stress-d3.mjs`
//! under Node, with the frozen force set from `graph_core::layout::force::ForceParams`
//! and the same golden-spiral seed positions our own port uses — so both sides start
//! from the same picture and only the *algorithm* differs. The metric itself is applied
//! to both arms' positions by [`metric`], once, in Rust.
//!
//! Exit codes follow graph-cli: 0 pass · 1 a case fell below the margin · 2 could not
//! run (no `node`, no resolvable `d3-force`, no fixtures). The verdict is recorded in
//! the record the measured layout's ledger row names (`<gates>/stress.json` for
//! `layout.force.barnes_hut`, `stress-pm.json` for `layout.force.particle_mesh`); a row may
//! only stand as `gated` with it present and current.

pub(crate) mod cases;
#[cfg(test)]
mod fa2;
pub(crate) mod metric;

use crate::evidence::Stamp;
use cases::Case;
use graph_core::layout::force::{BarnesHut, ForceParams, ParticleMesh};
use graph_core::{Geometry, Stage, StageError, Topology};
use metric::Graph;
use serde_json::json;
use std::process::ExitCode;

/// How far below d3's own correlation one case may sit and still pass.
pub const MARGIN: f64 = -0.05;

/// A layout this gate measures and the record it writes for it.
pub(crate) struct Subject {
    /// The registry's own id, so the ledger's `hash_stage` and this record cannot drift
    /// apart silently.
    pub(crate) stage: &'static str,
    /// The record the ledger's `oracle_record` names for that stage.
    pub(crate) record: &'static str,
    /// The stage at the frozen force set.
    pub(crate) run: Layout,
}

/// A force stage at given parameters.
pub type Layout = fn(&Topology, &ForceParams) -> Result<Geometry, StageError>;

/// Every layout `--layout` accepts; the first is the default.
pub(crate) const SUBJECTS: [Subject; 2] = [
    Subject {
        stage: BarnesHut::ID,
        record: "stress",
        run: BarnesHut::run,
    },
    Subject {
        stage: ParticleMesh::ID,
        record: "stress-pm",
        run: ParticleMesh::run,
    },
];

/// `graph-cli stress --oracle <name> --layout <id> --seeds <n>`.
pub fn run(oracle: &str, layout: &str, seeds: u32) -> ExitCode {
    let Some(subject) = SUBJECTS.iter().find(|s| s.stage == layout) else {
        eprintln!("stress: {layout:?} is not a layout this gate measures");
        return ExitCode::from(2);
    };
    if oracle != "d3" {
        eprintln!("stress: unknown oracle {oracle:?}: the only one wired is `d3`");
        return ExitCode::from(2);
    }
    if seeds == 0 {
        eprintln!("stress: 0 seeds: a margin over nothing proves nothing");
        return ExitCode::from(2);
    }
    match collect(subject, seeds) {
        Ok(collected) => report(subject, collected),
        Err(err) => {
            eprintln!("stress: could not run: {err}");
            ExitCode::from(2)
        }
    }
}

/// One seed's line: the simple graph both arms lay out, and each arm's correlation
/// over it.
#[derive(Debug, Clone)]
struct Seed {
    /// The correlation d3 reached, when one exists. A one-node case has no pair and is
    /// reported as absent rather than filled in.
    d3_r: Option<f64>,
    /// Ours.
    ours_r: Option<f64>,
}

/// Runs both arms over seeds `0..seeds` and correlates each case with [`metric`].
///
/// The scratch directory is named by process id so two `graph-cli stress` runs cannot
/// read each other's d3 output; it holds only the two exchanged JSONL files, which
/// `cases` re-derives on demand rather than trusting.
fn collect(subject: &Subject, seeds: u32) -> Result<Vec<Seed>, String> {
    let ours: Vec<Case> = cases::ours(seeds, subject.run)?;
    let scratch = std::env::temp_dir().join(format!("gm-stress-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let theirs = cases::d3(&scratch, &ours);
    std::fs::remove_dir_all(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
    let theirs: Vec<Case> = theirs?.into_iter().map(|(case, _)| case).collect();
    ours.iter()
        .zip(&theirs)
        .map(|(ours, theirs)| {
            let graph = Graph::from_edges(&ours.edges);
            Ok(Seed {
                d3_r: metric::correlate(&graph, &theirs.positions),
                ours_r: metric::correlate(&graph, &ours.positions),
            })
        })
        .collect()
}

/// Prints the per-case margins and the median, and records the verdict.
fn report(subject: &Subject, seeds: Vec<Seed>) -> ExitCode {
    let stamp = match Stamp::take() {
        Ok(stamp) => stamp,
        Err(err) => {
            eprintln!("stress: could not run: {err}");
            return ExitCode::from(2);
        }
    };
    println!(
        "stress: oracle=d3-force@3.0.0 stage={} seeds={} margin={MARGIN}",
        subject.stage,
        seeds.len()
    );
    let mut margins = Vec::new();
    let mut degenerate = 0usize;
    for (i, seed) in seeds.iter().enumerate() {
        match (seed.d3_r, seed.ours_r) {
            (Some(d3), Some(ours)) => {
                let margin = ours - d3;
                margins.push(margin);
                println!(
                    "  seed {i}: r(d3)={d3:.5} r(ours)={ours:.5} margin={margin:+.5}{}",
                    if margin < MARGIN {
                        "  BELOW MARGIN"
                    } else {
                        ""
                    }
                );
            }
            _ => {
                degenerate += 1;
                println!("  seed {i}: no correlation exists (a component with no pair)");
            }
        }
    }
    margins.sort_by(f64::total_cmp);
    let median = margins.get(margins.len() / 2).copied();
    let below: Vec<f64> = margins.iter().copied().filter(|m| *m < MARGIN).collect();
    if let Some(median) = median {
        println!(
            "  median margin={median:+.5} over {} correlated cases",
            margins.len()
        );
    }
    let pass = below.is_empty() && median.is_some_and(|m| m >= MARGIN);
    let body = json!({
        "seeds": seeds.len(),
        "pass": pass,
        "margin": MARGIN,
        "pivots": metric::PIVOTS,
        "metric": "Pearson correlation of BFS hop distance against euclidean distance, \
                   over 32 max-min pivots starting at dense index 0",
        "oracle": "d3-force@3.0.0, the frozen force set (link, manyBody, center, collide) \
                   at graph_core::layout::force::ForceParams' own values, 112 ticks, from the \
                   same golden-spiral seed positions (harness/stress-d3.mjs)",
        "correlated": margins.len(),
        "degenerate": degenerate,
        "median_margin": median,
        "worst_margin": margins.first().copied(),
        "functions": {
            subject.stage: {
                "cases": margins.len(),
                "declared": below.len(),
                "unexplained": below.len(),
            }
        },
    });
    if let Err(err) = crate::evidence::record(&stamp, subject.record, body) {
        eprintln!("stress: not recorded: {err}");
        return ExitCode::from(2);
    }
    if pass {
        println!("PASS");
        ExitCode::SUCCESS
    } else {
        println!(
            "FAIL: {} of {} cases below the {MARGIN} margin",
            below.len(),
            margins.len()
        );
        ExitCode::from(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::registry;

    /// The record this command writes for each layout must be the one that layout's
    /// ledger row names as its `oracle_record`, and the stage it measures must be the one
    /// the row is gated on. Either drifting apart would leave a row reading "not backed:
    /// no stress record" against a gate that passes.
    #[test]
    fn the_stage_and_the_record_are_the_ones_the_ledger_row_names() {
        let rows = registry();
        for subject in &SUBJECTS {
            let row = rows
                .iter()
                .find(|r| r.id == subject.stage)
                .expect("the measured layout is registered");
            assert_eq!(row.oracle_record, subject.record, "{}", subject.stage);
            assert_eq!(row.hash_stage, subject.stage);
            assert!(
                crate::hashgate::stages().contains(&subject.stage),
                "the 4-way gate must cover {}",
                subject.stage
            );
        }
    }

    /// Only `d3` and the layouts in [`SUBJECTS`] are wired; anything else is a refusal,
    /// never a silent pass.
    #[test]
    fn an_unknown_oracle_is_refused_rather_than_run() {
        let refused = ExitCode::from(2);
        for oracle in ["d3-force", "", "networkx", "D3"] {
            assert_eq!(
                run(oracle, BarnesHut::ID, 8),
                refused,
                "{oracle:?} must be refused"
            );
        }
        assert_eq!(
            run("d3", BarnesHut::ID, 0),
            refused,
            "0 seeds must be refused"
        );
        assert_eq!(run("d3", "layout.grid", 8), refused, "an unmeasured layout");
    }
}
