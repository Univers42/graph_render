//! The Graphviz layout engines ported into the motor, one module each.
//!
//! These are the engines whose oracle is Graphviz's **own** output rather than a Python
//! library: the reference is read as an algorithm reference and reimplemented, never
//! translated line by line and never linked (`docs/decisions/graphviz-oracle.md`, which
//! also fixes the units — Graphviz reports points, so these modules emit points and do
//! not rescale). `layout::radial::twopi` is the same family and predates this directory;
//! it stays where it is rather than moving, because a move would put a hash-gate stage's
//! module path in the diff for no behavioural change.

pub mod osage;
