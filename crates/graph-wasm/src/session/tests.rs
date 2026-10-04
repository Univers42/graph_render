//! This module's tests, split three ways by what each holds.
//!
//! [`fixture`] is the shared setup: the gate's own model, a live session over it, and one
//! column as the wire reads it. [`bits`] is the **load-bearing** half — N ticks driven through
//! this module's functions must be the same *bits* as `ForceSession::step(N)` called directly,
//! or "one tick, one pin, one column" is a claim about the code as written and not as shipped.
//! [`refusals`] is the id table (C6), the error each refusal maps to, and the two constants
//! the wire carries. [`threaded`] is the same bits claim for `tick_with` over the pool.
//!
//! All native (C21): no wasm build is in the loop. The wasm32 half of the same determinism
//! claim is `graph-cli force-gate`, which drives the exports themselves under Node.

mod bits;
mod fixture;
mod refusals;
mod threaded;
mod warm;
