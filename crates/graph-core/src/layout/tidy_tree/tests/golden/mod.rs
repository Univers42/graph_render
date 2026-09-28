//! Golden coordinates pinned against **d3-hierarchy@3.1.2** (the version vendored in
//! `node_modules/d3-hierarchy`, identical to
//! `/goinfre/dlesieur/refs/npm/d3-hierarchy-3.1.2/`), whose `src/tree.js` is the
//! Buchheim/Jünger/Leipert/Walker algorithm this crate ports.
//!
//! Every constant here was produced by *running* that oracle on the same tree, with the
//! call sequence `tidy_tree.rs`'s module doc states:
//!
//! ```js
//! d3.tree()(d3.hierarchy(data, n => n.children));   // every option at its default
//! ```
//!
//! The port computes in `f64` throughout and casts to `f32` only at the final write, so
//! the comparison is against `Math.fround` of d3's `f64` — hence the `f32` bit patterns,
//! asserted with `to_bits()`. The port claims exactness (its module doc owes no
//! Ponytail for the algorithm), so a single-ulp drift is a bug, not noise.
//!
//! These sit **on top of** the behavioural tests in the parent module — determinism,
//! finiteness, sibling ordering, same-depth `y` — so a golden that stopped matching could
//! not pass on its own.
//!
//! **The shapes are not chosen at random.** Each is the smallest tree a sweep of the real
//! port found that reaches a specific line of `walk.rs`, and the module is split so that
//! each group can be read on its own:
//!
//! - [`shapes`] — the ordinary shapes: a chain, a fan, a deep-unbalanced tree that makes
//!   `apportion` cross a subtree boundary, a caterpillar, and both single-root fixtures.
//! - [`witnesses`] — the three shapes that reach `execute_shifts` at all, and the
//!   `m[w] += shift` line in particular.
//! - [`ties`] — the shapes that make `normalize`'s strict `<` / `>` observable, plus
//!   `finish_contour`'s second trailing `if`.

use super::{build, nodes, tree};

/// `(id, x, y)` as `f32` bit patterns, in whatever order a shape reads best.
pub(super) type Row = (&'static str, u32, u32);

mod cases;
mod helpers;
mod ordinary;
mod shapes;
mod ties;
mod witnesses;

pub(super) use helpers::{assert_golden, assert_indexed, dense_ids};
pub(super) use shapes::{CATERPILLAR9, CHAIN8, FAN10, TREE_BALANCED, TREE_DEGENERATE, UNBALANCED};
pub(super) use ties::{FINISH_SECOND_BRANCH, LEFT_TIE, RIGHT_TIE};
pub(super) use witnesses::{EXECUTE_SHIFTS_WITNESS, M_SHIFT_WITNESS, SIGN_FLIP_WITNESS};
