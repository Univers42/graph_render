//! The execution tiers: how the work is divided, and which division runs.
//!
//! Phase 11 (`prompts/phase-11-compute-tiers.md`, `docs/decisions/compute-tiers.md`). Two
//! halves, both pure and both in graph-core:
//!
//! - [`partition`] — **where** the slices are, and [`Runner`]/[`Serial`] — **who** runs
//!   them. The range-kernel contract's two halves: a kernel has the shape `fn step_range(&
//!   self, range, out)`, reads only start-of-step state and writes only `out[range]`, so
//!   any division of `0..n` is a legal executor's plan and none of them changes a byte.
//! - [`select`] — **which** division runs, from `(n, m, caps, thresholds)` alone.
//!
//! **The executors live outside this crate** (`compute-tiers.md` rule 2): graph-core never
//! spawns a thread, detects hardware or reads a clock, so it still compiles for
//! `wasm32-unknown-unknown` and still has no dependency beyond its own. Native threads are
//! `crates/graph-cli/src/exec_native.rs`'s `std::thread::scope`; browser workers are the
//! SDK's. Each is a [`Runner`]: it calls [`partition`] and hands each range to a kernel;
//! none adds a term to a sum or reorders one (D3), which is why a tier is byte-identical
//! rather than close. [`Serial`] is graph-core's own runner, and the reference every other
//! one is compared against.

mod partition;
mod select;

pub use partition::{Runner, Serial, StepRange, partition, range_at};
pub use select::{Caps, Exec, SelectError, Thresholds, Tier, resolve, select};
