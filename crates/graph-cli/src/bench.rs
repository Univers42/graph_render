//! `graph-cli bench --n 220,10000,100000`: the measured numbers behind the two force
//! layouts' registered `scale_ceiling`.
//!
//! The phase prompt is blunt about what a force-layout speed claim may rest on: d3's
//! `forceManyBody` **already** uses a quadtree, so porting Barnes-Hut is not an
//! asymptotic win but a constant-factor one, and "O(n log n) instead of O(n^2)" is an
//! O-notation argument, which is not evidence. The justification has to be a measured
//! number at N = 220 / 10 000 / 100 000 — and N = 220 is today's real graph size, the
//! one most likely to show the port *losing*, which is exactly why it is measured first
//! and reported even when unflattering.
//!
//! Two things this command deliberately does:
//!
//! - **It times `Stage::run` and nothing else.** Building the synthetic model and
//!   formatting the output are outside the timer, so the number is the layout's.
//! - **It refuses a size past a layout's own registered ceiling** rather than running
//!   for minutes. The ceiling is a budget; a command that silently blows through its
//!   own budget is not a gate. A refusal names the ceiling it came from, so the
//!   ceiling is checkable rather than a number in a doc.
//!
//! It is not a claim about wasm32: those numbers are native and this build is debug
//! unless `--release` is passed. Cross-target speed is Phase 9's, per
//! `docs/measurements/phase06-force.md`.

use graph_core::registry::{FA2_CEILING, FORCE_CEILING};
use graph_core::{REFERENCE_DEGREE, Stage, index_model, seeded_model};
use std::process::ExitCode;
use std::time::Instant;

/// The sizes the phase gate names. A caller may pass others; these are the ones the
/// ceiling derivations and `docs/measurements/phase06-force.md` quote.
const GATE_SIZES: &str = "220,10000,100000";

/// The layouts this bench times, with the ceiling each one's `scale_ceiling` states.
const TARGETS: [(&str, u64); 2] = [
    ("layout.force.barnes_hut", FORCE_CEILING),
    ("layout.forceatlas2", FA2_CEILING),
];

/// `graph-cli bench --n <list> [--dry-run]`.
pub fn run(sizes: &str, dry_run: bool) -> ExitCode {
    let sizes = match parse(sizes) {
        Ok(sizes) => sizes,
        Err(err) => {
            eprintln!("bench: {err}");
            return ExitCode::from(2);
        }
    };
    let mut refused = false;
    for n in sizes {
        println!("n={n}");
        for (id, ceiling) in TARGETS {
            if u64::from(n) > ceiling {
                println!("  {id:<26} refused: n={n} is past its scale_ceiling of {ceiling}");
                refused = true;
                continue;
            }
            if dry_run {
                println!("  {id:<26} would run (scale_ceiling {ceiling})");
                continue;
            }
            let (nodes, edges) = seeded_model(0, n, REFERENCE_DEGREE);
            let topology = match index_model(&nodes, &edges) {
                Ok(topology) => topology,
                Err(err) => {
                    eprintln!("bench: n={n}: {err}");
                    return ExitCode::from(2);
                }
            };
            if let Some(elapsed) = time(id, &topology) {
                println!(
                    "  {id:<26} {elapsed:>10.2} ms  (edges={})",
                    topology.edge_count()
                );
            }
        }
    }
    if refused {
        // A refusal is the command working, not failing: the ceiling did its job, and
        // the phase gate's own size set (220, 10000, 100000) includes one that FA2
        // refuses. It is reported so a caller who passed a size past a ceiling is
        // never left guessing whether it ran.
        println!("bench: one or more sizes were past a layout's own scale_ceiling");
    }
    ExitCode::SUCCESS
}

/// One layout's `Stage::run`, timed, over the topology. `None` when the stage refused
/// (a non-finite position), which is reported as a refusal rather than a time.
fn time(id: &str, topology: &graph_core::Topology) -> Option<f64> {
    use graph_core::layout::force::{BarnesHut, ForceParams};
    use graph_core::layout::forceatlas2::{Fa2Params, ForceAtlas2};
    let (start, result) = match id {
        "layout.force.barnes_hut" => {
            let params = ForceParams::default();
            (Instant::now(), BarnesHut::run(topology, &params))
        }
        _ => {
            let params = Fa2Params::default();
            (Instant::now(), ForceAtlas2::run(topology, &params))
        }
    };
    let elapsed = start.elapsed().as_secs_f64() * 1000.0;
    match result {
        Ok(_) => Some(elapsed),
        Err(err) => {
            println!("  {id:<26} refused: {err}");
            None
        }
    }
}

/// A comma-separated list of node counts, each at least 1.
fn parse(sizes: &str) -> Result<Vec<u32>, String> {
    if sizes.trim().is_empty() {
        return Err(format!(
            "--n takes a comma-separated list, e.g. {GATE_SIZES}"
        ));
    }
    sizes
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<u32>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| format!("--n {part:?} is not a node count of at least 1"))
        })
        .collect()
}

/// The stage ids this bench times, checked against the registry so a renamed or
/// unregistered layout cannot leave the bench quietly measuring nothing.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_benched_layout_is_registered_with_the_ceiling_the_bench_uses() {
        for (id, ceiling) in TARGETS {
            let layout =
                graph_core::registry::find(id).unwrap_or_else(|| panic!("{id} is not registered"));
            assert_eq!(layout.meta.scale_ceiling, ceiling, "{id}");
        }
    }

    #[test]
    fn sizes_parse_and_a_typo_is_refused_rather_than_defaulted() {
        assert_eq!(
            parse("220,10000, 100000").expect("parses"),
            [220, 10000, 100000]
        );
        assert_eq!(parse(" 40 ").expect("parses"), [40]);
        for bad in ["", " ", "0", "-1", "abc", "40,0", "40,,60", "1e3", "40;60"] {
            assert!(parse(bad).is_err(), "{bad:?} must be refused");
        }
    }
}
