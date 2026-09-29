//! `graph-cli bench`: what the plan resolves to, and what one row of it means.
//!
//! Four child modules, one per claim, so each stays inside the house's 300-line cap:
//!
//! - [`plan`] — the flag's own vocabulary: which layouts, which ceiling, and what a
//!   refusal versus a failure is.
//! - [`harness`] — the two JS arms' self-checks, each with the negative control that says
//!   the self-check can fail.
//! - [`ladder`] — the crossover table: a `Sample`, an arm reading, and the arithmetic that
//!   turns them into markdown.
//! - [`scale`] — the scale fixtures: the synthetic model itself, past the cap, and the
//!   ingest shape the emitted document must have.

mod harness;
mod ladder;
mod plan;
mod scale;

// The negative controls' staged mutant copies, and the test that pins where a
// transient file may be written, live in `bench/staging.rs`.
