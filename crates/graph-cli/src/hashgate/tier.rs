//! The gate's execution tiers, as **arm names**. One list, parsed once, and every arm the
//! gate runs is one entry of it — so "which arms does `--tiers all` add" has one answer in
//! the code rather than one in the flag parser and another in the report.
//!
//! The odd worker counts are here and not in a benchmark script: 1, 2, 3, 4 and 7 leave
//! uneven slices, and an even split hides a range-boundary bug behind symmetry. A gate that
//! only ever ran 1, 2 and 4 would have passed a kernel that dropped the last node of every
//! even-length slice.

use super::Arm;

/// One tier the gate can run an arm under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// The compiled-in scalar path: one thread, one process.
    Scalar,
    /// Every threaded stage — the three force layouts' range kernels, the grid's gather, and
    /// the ring's and the spiral's — over this many `std::thread`s.
    Threads(u32),
}

impl Tier {
    /// The arm's name in the report: short, and naming the worker count, because a
    /// divergence report that said "threads" without saying how many would send the reader
    /// back to the flag to find out which plan broke.
    pub const fn arm_name(self) -> &'static str {
        match self {
            Tier::Scalar => "native scalar",
            // The names are spelled out rather than formatted, because `Arm`'s name is a
            // `&'static str` and a runtime `format!` would allocate a leaked string per
            // arm per run to save a match arm.
            Tier::Threads(1) => "native threads 1",
            Tier::Threads(2) => "native threads 2",
            Tier::Threads(3) => "native threads 3",
            Tier::Threads(4) => "native threads 4",
            Tier::Threads(7) => "native threads 7",
            Tier::Threads(_) => "native threads other",
        }
    }
}

/// The worker counts the threaded arms use. Pinned rather than derived from the host's
/// core count on purpose: a gate that added an arm per core would be a different gate on a
/// loaded machine, and `threads 5` proves nothing that `threads 4` and `threads 7` between
/// them do not.
pub const WORKER_COUNTS: [u32; 5] = [1, 2, 3, 4, 7];

/// Which arms a run takes, from `--tiers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tiers {
    /// The four original arms and nothing else: native ×2 and wasm32 ×2.
    Base,
    /// The four plus one native arm per worker count in [`WORKER_COUNTS`].
    All,
}

impl Tiers {
    /// The word as it is typed, so an unknown one can name itself in the error.
    pub const fn as_str(self) -> &'static str {
        match self {
            Tiers::Base => "base",
            Tiers::All => "all",
        }
    }
}

/// The value `--tiers` accepts, or the error naming what it accepts.
pub fn parse(text: &str) -> Result<Tiers, String> {
    match text {
        "base" => Ok(Tiers::Base),
        "all" => Ok(Tiers::All),
        other => Err(format!(
            "--tiers {other:?}: expected {}",
            [Tiers::Base.as_str(), Tiers::All.as_str()].join(" or ")
        )),
    }
}

/// The tiered native arms this run adds, each already run over `seeds` seeds.
///
/// `Scalar` comes first and is the reference: the report's per-stage count is
/// "N-way equal", and the first arm is the one every other is compared against.
pub fn arms(seeds: u32, tiers: Tiers) -> Result<Vec<Arm>, String> {
    let mut out = Vec::new();
    if tiers == Tiers::All {
        out.push(scalar_arm(seeds)?);
        for workers in WORKER_COUNTS {
            out.push(threads_arm(seeds, workers)?);
        }
    }
    Ok(out)
}

/// The compiled-in scalar arm, as a [`crate::hashgate::Arm`].
///
/// Its lines come from [`crate::hashgate::arm_lines`] split on newlines, the same split the
/// child-process arms go through, so an arm built in-process and one read off a pipe are
/// the same list of strings and `compare` cannot tell them apart.
pub fn scalar_arm(seeds: u32) -> Result<Arm, String> {
    let setting = super::env_setting()?;
    let printed = super::arm_lines(seeds, &setting)?;
    Ok((
        Tier::Scalar.arm_name(),
        printed.lines().map(str::to_owned).collect(),
    ))
}

/// The threaded stages over `workers` `std::thread`s.
///
/// **Only the threaded stages change**: every other stage runs the same bytes whichever
/// tier the graph is at, so re-running them per tier would buy no coverage and cost one
/// full pipeline per worker count. The claim being checked is precisely that each threaded
/// stage is worker-count-invariant — the three force layouts' three range kernels, the
/// grid's gather, and the ring's and the spiral's gather over the shared serial `coords`
/// merge — and every other stage is already covered by the four base arms. The list itself
/// lives in [`super::threaded_bytes`]'s match, so the two cannot disagree.
pub fn threads_arm(seeds: u32, workers: u32) -> Result<Arm, String> {
    let setting = super::env_setting()?;
    Ok((
        Tier::Threads(workers).arm_name(),
        super::threads_lines(seeds, &setting, workers)?,
    ))
}

#[cfg(test)]
mod tests;
