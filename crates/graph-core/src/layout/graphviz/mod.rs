//! Ports of **Graphviz's own** layout engines, the row `docs/decisions/graphviz-oracle.md`
//! commits to matching bit for bit rather than re-deriving.
//!
//! `layout::radial::twopi` is the other one, and it predates this module: it is a
//! *radial* engine that computes in inches and reports points, so it sits with the other
//! radial layouts. What is here is the **neato family** — engines whose output is
//! `ND_pos` in the reference's own dimensionless units, and whose answers depend on a
//! seeded generator, so a port of one is a port of a whole arithmetic rather than of a
//! formula.
//!
//! One rule governs every module in here, and it is the reason the code is shaped the way
//! it is: **the reference's arithmetic is reproduced, not improved on.** Graphviz's
//! stress majorization carries coordinates in `float` and scalars in `double`, accumulates
//! the Laplacian diagonal in `long double`, and reaches its answer through a conjugate
//! gradient whose stopping rule is a tolerance. Every one of those choices is load-bearing
//! for agreement, and none of them is one a reader would write on purpose, so each is
//! named where it is reproduced and the reason is recorded next to it. An `f64`-only port
//! is a *different algorithm* that agrees in the limit, and the differential measures the
//! limit, not the port.

pub mod neato;
