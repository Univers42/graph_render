//! The Graphviz layout engines ported into the motor, one module each.
//!
//! These are the engines whose oracle is Graphviz's **own** output rather than a Python
//! library: the reference is read as an algorithm reference and reimplemented, never
//! translated line by line and never linked (`docs/decisions/graphviz-oracle.md`, which
//! also fixes the units — Graphviz reports points, so these modules emit points and do
//! not rescale). `layout::radial::twopi` is the same family and predates this directory;
//! it stays where it is rather than moving, because a move would put a hash-gate stage's
//! module path in the diff for no behavioural change.
//!
//! Every engine in this tree is compared against the docker-only Graphviz oracle rather
//! than against a second run of ours, and every one of them is a port read from the
//! pinned Graphviz release as an algorithm reference — never a translation, never a link.
//!
//! **The reference's arithmetic is reproduced, not improved on.** Graphviz carries
//! coordinates in `float` and scalars in `double`, accumulates the Laplacian diagonal in
//! `long double`, and reaches its answer through a conjugate gradient whose stopping rule
//! is a tolerance. Every one of those choices is load-bearing for agreement and none is
//! one a reader would write on purpose, so each is named where it is reproduced and the
//! reason recorded next to it. An `f64`-only port is a *different algorithm* that agrees
//! in the limit, and the differential measures the limit, not the port.

pub mod circo;
pub mod dot;
pub mod fdp;
pub mod neato;
pub mod osage;
pub mod patchwork;
pub mod sfdp;
pub mod text_width;
