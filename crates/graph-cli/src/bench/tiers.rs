//! The tier sweep: the same graph, the same layout, timed under every execution tier the
//! host actually offers — the numbers `docs/decisions/tier-thresholds.md`'s rows are
//! derived from, and the only place a tier's speedup is claimed.
//!
//! The sweep measures the **whole Barnes-Hut stage**, not one pass: a threshold selects a
//! tier for a layout run, so the number that justifies a row is the run's, and a per-pass
//! number would be a measurement of something no host runs. Which passes the stage hands
//! to the runner is stated in the report, because it is the denominator of every speedup
//! here (Amdahl): a stage with one pass threaded cannot show more than that pass's share.
//!
//! Two rules the sweep holds itself to:
//!
//! - **Every arm is compared with the serial arm's bytes.** A faster tier that produced
//!   different geometry would be a faster wrong answer, so a cell carries an `equal` flag
//!   read against the same run's serial arm, and the sweep fails when a cell says false.
//! - **The host is reported with the numbers.** `nproc`, the load average at the start
//!   and at the end, the repeat count and every individual run. A speedup measured on a
//!   loaded host is not a smaller speedup, it is a different number, and a table without
//!   the load next to it cannot be read honestly.

pub mod markdown;
mod sweep;

pub use sweep::run;
/// The control entry point, reachable only from this module's own tests: a host has no
/// business timing a deliberately wrong tier, and a public one could be taken for a flag.
#[cfg(test)]
pub use sweep::run_under;

#[cfg(test)]
mod tests;

use crate::bench::Plan;

/// The worker counts `threads` is timed at, spelled the way `bench --workers` parses them.
///
/// The array below is the list; this string is what clap's `default_value` needs, and
/// `the_default_worker_list_is_the_list` holds the two together, because a default that
/// had drifted from the pinned list would quietly measure a different set of widths than
/// the one the gate proves equal.
pub const WORKERS_DEFAULT: &str = "2,4,7";

/// The worker counts `threads` is timed at.
///
/// Pinned here and not derived from the host's core count: a sweep that added an arm per
/// core would be a different table on every machine, and the point of the list is that it
/// can be compared with a run taken weeks later on the same host. Every count is one
/// `hashgate --tiers all` arm runs, so no row here rests on a width nothing has proved
/// equal; [`entry`] refuses a `--workers` outside that list. One is absent: the scalar arm
/// is in the same sweep and *is* the one-worker run.
pub const WORKER_COUNTS: [u32; 3] = [2, 4, 7];

/// A tier the sweep can time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The compiled-in path: one thread, one worker.
    Scalar,
    /// Barnes-Hut's range kernels over this many `std::thread`s.
    Threads(u32),
}

impl Tier {
    /// The arm's name in the table: the worker count is part of the name, because a row
    /// that said only "threads" would not say what was compared.
    pub const fn arm_name(self) -> &'static str {
        match self {
            Tier::Scalar => "scalar",
            Tier::Threads(2) => "threads 2",
            Tier::Threads(4) => "threads 4",
            Tier::Threads(7) => "threads 7",
            Tier::Threads(_) => "threads other",
        }
    }
}

/// A tier `--tiers` asks for, before the worker count is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Asked {
    Scalar,
    Threads,
}

/// The tiers this build can time. `simd` and `gpu` are **not** here, and asking for one
/// is refused by name rather than quietly replaced by scalar: a caller who asked for a
/// tier that does not exist has been lied to otherwise.
fn one(word: &str) -> Result<Asked, String> {
    match word {
        "scalar" => Ok(Asked::Scalar),
        "threads" => Ok(Asked::Threads),
        other => Err(format!(
            "--tiers {other:?}: this build times scalar and threads only \
             (exec.simd and the GPU tier are not built)"
        )),
    }
}

/// The `--tiers` list, comma separated. An unknown word is an error, never a default.
pub fn parse_asked(text: &str) -> Result<Vec<Asked>, String> {
    text.split(',').map(|word| one(word.trim())).collect()
}

/// The same list as `bench`'s flag parser: clap vets the word, this maps it. `--tiers`
/// splits on commas before the parser sees a value, so this runs once per word and yields
/// that word's one tier.
pub fn parse_asked_list() -> impl clap::builder::TypedValueParser {
    use clap::builder::TypedValueParser as _;
    clap::builder::PossibleValuesParser::new(["scalar", "simd", "threads", "gpu"]).try_map(|word| {
        let mut asked = parse_asked(word.as_str())?;
        match asked.len() {
            1 => Ok(asked.remove(0)),
            _ => Err(format!("--tiers {word:?}: expected one tier, not a list")),
        }
    })
}

/// The arms `asked` names: `threads` over every width in `workers`, in the order given.
pub fn arms(asked: &[Asked], workers: &[u32]) -> Vec<Tier> {
    let mut out = Vec::new();
    for ask in asked {
        match ask {
            Asked::Scalar => out.push(Tier::Scalar),
            Asked::Threads => out.extend(workers.iter().map(|&w| Tier::Threads(w))),
        }
    }
    out
}

/// One timed (size, tier) cell: every run, and whether the arm's geometry was the serial
/// arm's bytes.
#[derive(Debug, Clone)]
pub struct Cell {
    pub n: u32,
    pub tier: Tier,
    pub runs_ms: Vec<f64>,
    pub equal: bool,
}

impl Cell {
    /// The cell's median wall milliseconds — a median over the repeats, never one timing.
    pub fn median_ms(&self) -> f64 {
        super::campaign::median(self.runs_ms.clone())
    }

    /// This arm's speedup against the serial arm at the same size, or `None` for the
    /// serial arm itself: a tier's speedup is a claim about a *difference*, and the
    /// reference has none.
    pub fn speedup(&self, reference: &Cell) -> Option<f64> {
        if self.tier == Tier::Scalar || self.median_ms() <= 0.0 {
            return None;
        }
        Some(reference.median_ms() / self.median_ms())
    }
}

/// The machine the numbers came off: core count and the load at both ends of the run.
#[derive(Debug, Clone)]
pub struct Host {
    pub nproc: usize,
    pub load_start: String,
    pub load_end: String,
}

/// The bracket the crossover sits in: the largest measured size at which **every** threaded
/// arm lost to scalar, and the smallest at which **any** won. `None` when the arms won (or
/// lost) at every measured size, because then there is no crossover in this table to write
/// a threshold row from.
///
/// It is a bracket and not a point because the sweep measures the sizes it was asked for:
/// a crossover between two measured sizes is reported as the two sizes, and the threshold
/// row takes the **first winning** one — the smallest size at which the tier was measured
/// to be the faster choice, which is the only statement the data actually supports.
pub fn crossover(cells: &[Cell]) -> Option<(u32, u32)> {
    let mut last_loss: Option<u32> = None;
    let mut first_win: Option<u32> = None;
    for n in cells
        .iter()
        .map(|c| c.n)
        .collect::<std::collections::BTreeSet<_>>()
    {
        let at: Vec<&Cell> = cells.iter().filter(|c| c.n == n).collect();
        let (Some(reference), _) = (at.iter().find(|c| c.tier == Tier::Scalar), ()) else {
            continue;
        };
        let speeds: Vec<f64> = at
            .iter()
            .filter(|c| c.tier != Tier::Scalar)
            .filter_map(|c| c.speedup(reference))
            .collect();
        if speeds.is_empty() {
            continue;
        }
        let won = speeds.iter().any(|s| *s > 1.0);
        if won {
            first_win = first_win.or(Some(n));
        } else {
            last_loss = Some(n);
        }
    }
    match (last_loss, first_win) {
        (Some(loss), Some(win)) if loss < win => Some((loss, win)),
        _ => None,
    }
}

/// The widths to time: the plan's own, or the pinned list when it named none. A caller
/// that asked for `threads` and gave no width gets the pinned list rather than no
/// threaded arm at all, which would silently time one tier and call it a sweep.
fn widths(plan: &Plan) -> Vec<u32> {
    if plan.workers.is_empty() {
        WORKER_COUNTS.to_vec()
    } else {
        plan.workers.clone()
    }
}

/// Every width the plan asks for is one the gate has proved hash-equal. A number measured
/// at a width nothing was compared at cannot become a threshold row, so the sweep refuses
/// it here rather than writing it into a table someone reads later.
fn refuse_unproved_widths(widths: &[u32]) -> Result<(), String> {
    for &workers in widths {
        if !crate::hashgate::WORKER_COUNTS.contains(&workers) {
            return Err(format!(
                "--workers {workers}: hashgate --tiers all runs {:?}",
                crate::hashgate::WORKER_COUNTS
            ));
        }
    }
    Ok(())
}

/// `bench --tiers`: measure, print a row per cell, write the report, and answer whether
/// every arm produced the serial arm's bytes.
pub fn entry(plan: &Plan) -> Result<bool, String> {
    let widths = widths(plan);
    refuse_unproved_widths(&widths)?;
    let tiers = arms(&plan.tiers.clone().unwrap_or_default(), &widths);
    if tiers.is_empty() {
        return Err("--tiers named no tier to time".into());
    }
    let (cells, host) = run(plan, &tiers)?;
    for cell in &cells {
        println!(
            "n={n} {tier:<10} {ms:>10.2} ms  equal to scalar: {equal}",
            n = cell.n,
            tier = cell.tier.arm_name(),
            ms = cell.median_ms(),
            equal = cell.equal
        );
    }
    if let Some(path) = markdown::report_path(plan) {
        super::campaign::report::write_report(path, &markdown::markdown(plan, &cells, &host))?;
    }
    Ok(cells.iter().all(|cell| cell.equal))
}
