//! [`partition`]'s rule, the [`StepRange`] contract, and the [`Serial`] runner — plus the
//! negative control that says the equality check is worth running.
//!
//! Three child modules, one per claim, so each stays inside the house's 300-line cap:
//!
//! - [`rule`] — `partition(n, workers)`: where the slices are. A pure function of two
//!   integers, so it is pinned by exact vectors rather than by properties.
//! - [`contract`] — the [`StepRange`] shape: any division of the outputs is the same
//!   computation, and the control that proves the equality check can go red.
//! - [`runner`] — [`Serial`]: the reference every other executor is measured against.

mod contract;
mod rule;
mod runner;
