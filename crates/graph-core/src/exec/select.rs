//! Which tier runs, decided by arithmetic and nothing else.
//!
//! [`select`] is a **pure function** of `(n, m, caps, thresholds)` (`compute-tiers.md`
//! rule 4, `phase-11-compute-tiers.md` step 5). No clock (D8): the thresholds carry the
//! measurements, and the host or SDK carries the capability flags — graph-core never
//! detects hardware and never reads elapsed time. Same inputs, same tier, every run.
//!
//! Three rules bind it, and each is a test below:
//!
//! 1. **`auto` never selects a GPU id** (`compute-tiers.md` rule 5). A GPU result is only
//!    per-device reproducible, so it is reached by naming it, never by being large.
//! 2. **An explicit request for an unavailable tier is refused**, never silently
//!    downgraded — a caller who asked for threads and got scalar has been lied to.
//! 3. **A tier past its threshold is not selected**, whatever the host reports. The
//!    thresholds are where the tier measured faster, so below one it is the slower choice.

use core::fmt;

/// What the host or SDK reports it can do. An input, never detected in here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// Whether a SIMD build of the module is in hand (wasm `simd128`, or SSE2/NEON native).
    pub simd: bool,
    /// How many workers the host will run at once; 0 or 1 means threads cannot help.
    pub workers: u32,
    /// Whether a WebGPU adapter exists.
    pub webgpu: bool,
}

impl Caps {
    /// Nothing available: the tier-1a motor, which every host can run.
    pub const NONE: Caps = Caps {
        simd: false,
        workers: 0,
        webgpu: false,
    };

    /// Whether `tier` could run here at all, before any threshold is consulted.
    pub const fn can_run(self, tier: Tier) -> bool {
        match tier {
            Tier::Scalar => true,
            Tier::Simd => self.simd,
            Tier::Threads(_) => self.workers > 1,
            Tier::Gpu => self.webgpu,
        }
    }
}

/// The tier thresholds, in nodes, taken from `docs/decisions/tier-thresholds.md`.
///
/// Every field is public and every value is a plain integer: the table is data the docs
/// derive from measurements, not a rule buried in a comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Thresholds {
    /// Nodes past which the SIMD tier measured faster than scalar.
    pub simd_nodes: u64,
    /// Nodes past which threads measured faster than the best single-threaded tier.
    pub threads_nodes: u64,
    /// Workers threads are capped at, whatever `caps.workers` reports.
    pub threads_max: u32,
}

impl Thresholds {
    /// The table as committed in `docs/decisions/tier-thresholds.md`.
    ///
    /// **`threads_nodes` and `threads_max` are measured** (`docs/measurements/phase11-threads.md`,
    /// `graph-cli bench --tiers scalar,threads --repeat 5`, release, 16-core host): threads
    /// lose at n=220 (0.52x..0.75x — the partitions' own cost with nothing to overlap) and
    /// win from n=10 000 (1.34x at two workers, 2.97x at seven), so the row is the smallest
    /// measured size at which they won. **`simd_nodes` is not measured**: no SIMD arm exists,
    /// so it stays at `u32::MAX + 1`, one past the largest node count a `u32` index can
    /// hold, and no graph reaches it (the comparison is `>=`).
    ///
    /// Two independent guards on the row that is not measured, so neither alone is
    /// load-bearing: `4_294_967_296` is unreachable by any node count, and `simd` is
    /// additionally refused for a host that reports no SIMD build.
    ///
    /// A speedup measured on one host is a sample of one host, which is why the table's
    /// Ponytail lives in `tier-thresholds.md` next to the numbers: a wrong row costs time,
    /// never bytes, because every tier it can select is hash-equal to scalar per stage.
    pub const MEASURED: Thresholds = Thresholds {
        simd_nodes: 4_294_967_296,
        threads_nodes: 10_000,
        threads_max: 7,
    };
}

/// The tier that runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Tier 1a: one thread, scalar, no SIMD lanes. Always available.
    Scalar,
    /// Tier 1b: vectorised across outputs, byte-identical to [`Tier::Scalar`].
    Simd,
    /// Tier 2: a fixed partition of the outputs over this many workers, byte-identical
    /// to [`Tier::Scalar`]. The count is the one that will run, not a request.
    Threads(u32),
    /// Tier 3: WebGPU compute, per-device reproducible only, never auto-selected.
    Gpu,
}

/// What a caller asked for: `Auto` lets [`select`] choose, anything else is named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exec {
    /// Pick by threshold. The default, and the only value that can refuse nothing.
    Auto,
    /// A named tier, or [`Tier::Threads`] for the host's own worker count.
    Named(Tier),
}

/// Why a named tier could not run. Never a downgrade: the caller is told, not served a
/// different tier than the one they named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectError {
    /// The host does not have what the named tier needs.
    Unavailable {
        /// The tier that was asked for.
        tier: Tier,
        /// What the host reported.
        caps: Caps,
    },
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { tier, caps } => write!(
                f,
                "{tier:?} is not available here (simd {}, workers {}, webgpu {})",
                caps.simd, caps.workers, caps.webgpu
            ),
        }
    }
}

/// The tier `auto` runs at `n` nodes and `m` edges under `caps`, by `thresholds`.
///
/// Scalar until a threshold is crossed, and never the GPU: see the module doc.
pub fn select(n: u64, m: u64, caps: Caps, thresholds: Thresholds) -> Tier {
    let _ = m;
    // The worker count is the *smaller* of what the host offers and what the table allows,
    // and threads are offered only if that count is still more than one. A table with
    // `threads_max: 1` therefore cannot select threads at any size, which is what makes the
    // provisional table below inert rather than merely unlikely.
    let workers = caps.workers.min(thresholds.threads_max);
    if n >= thresholds.threads_nodes && workers > 1 {
        return Tier::Threads(workers);
    }
    if n >= thresholds.simd_nodes && caps.simd {
        return Tier::Simd;
    }
    Tier::Scalar
}

/// The tier for `exec`: [`Exec::Auto`] defers to [`select`], a named tier is honoured or
/// refused.
pub fn resolve(
    exec: Exec,
    n: u64,
    m: u64,
    caps: Caps,
    thresholds: Thresholds,
) -> Result<Tier, SelectError> {
    match exec {
        Exec::Auto => Ok(select(n, m, caps, thresholds)),
        Exec::Named(tier) if caps.can_run(tier) => Ok(tier),
        Exec::Named(tier) => Err(SelectError::Unavailable { tier, caps }),
    }
}

#[cfg(test)]
mod tests;
