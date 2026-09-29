//! The SCALE stage (`prompts/phase-09-scale-bench.md`): what a front should draw at this
//! size ([`lod`]), what may be taken out of the picture and put back ([`simplify`]), and
//! how many iterations a graph of this size earns ([`adaptive`]).
//!
//! Three properties hold across all three modules, and they are the phase's real content:
//!
//! - **Nothing is destroyed.** [`lod`] emits masks and a tier, never a mutated topology;
//!   [`simplify`] records every node and edge it hid in a journal and restores them on
//!   request. An irreversible simplification is data loss wearing an optimisation's name.
//! - **Nothing reads a clock.** [`adaptive`]'s budget is a pure function of `(n, m)`;
//!   D8 forbids wall-clock in the motor, and the phase's gate greps for it.
//! - **Nothing depends on iteration order.** Fixed-order reductions, ties broken by dense
//!   index, no `HashMap` in any output path — so LOD hints and simplification journals
//!   are as reproducible as a layout's positions.

pub mod adaptive;
pub mod lod;
mod simple;
pub mod simplify;
