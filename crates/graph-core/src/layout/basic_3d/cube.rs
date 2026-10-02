//! `_cube_layout` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:83-103`): the
//! eight cube corners first, in the reference's literal order, then the remainder
//! scattered strictly inside. Closed form for the corners, a stream for the interior.
//!
//! The construction, as the reference writes it:
//!
//! ```text
//! n == 1            -> the origin, all three columns 0          (basic.py:88-89)
//! idx = min(n, 8)
//! positions[:idx]   = corners[:idx] * scale                     (basic.py:91-97)
//! positions[idx:]   = rng.uniform(-1, 1, (n-idx, 3)) * scale*0.8 (basic.py:99-101)
//! ```
//!
//! **The corner order is the layout.** Those eight triples are a literal array
//! (`basic.py:91-94`), not a computed cube, and their order is not a rotation or a
//! relabelling — it decides which node sits at which corner. Any reordering of the array
//! is a **visible regression, not a refactor**: the drawing is the same cube and different
//! nodes, so nothing about the picture would tell a reader, and every hashed snapshot
//! would move. It is pinned by `the_eight_corners_are_the_reference_literal_in_its_order`
//! as the exact eight triples in the exact eight slots.
//!
//! Which eight triples they are, in order: `(1,1,1) (1,-1,-1) (-1,1,-1) (-1,-1,1)
//! (-1,-1,-1) (-1,1,1) (1,-1,1) (1,1,-1)`. That is a fixed, deliberately ungeometric
//! sequence — it is not the `(+,+,+), (-,+,+), ...` sign enumeration a reader would write
//! from scratch, which is exactly why it is transcribed and pinned rather than derived.
//!
//! **The interior scatter is the reference's, draw for draw.** The corners were always a
//! closed form; the interior is three `RandomState` draws per node, so reproducing it means
//! reproducing *numpy's generator*, not just a uniform distribution.
//!
//! **The stream is MT19937 seeded from the layout seed.** The reference draws from
//! `_get_layout_rng()` (`common.py:43-52`), a `np.random.RandomState` built from
//! `get_layout_seed()` = `derive_seed(42, "layout")` = [`SEED`], and that is
//! [`Mt19937`](crate::rng::Mt19937) ported exactly — the legacy `init_genrand` recurrence
//! and the 53-bit `random_sample`. The crate's own
//! [`Mulberry32`](crate::synthetic::Mulberry32) **cannot** answer this row, and the reason
//! is worth stating because it is invisible in the picture: a different generator at the
//! same seed produces a scatter that is uniformly distributed in the same shell and shares
//! not one coordinate with the reference. Before this port that was the written state of
//! the row, and it was the reason the interior was compared statistically.
//!
//! **The layout RNG is reset before every layout call, not carried across them.**
//! `apply_graph_layout` calls `_reset_layout_rng()` on entry (`dispatcher.py:22`), and that
//! rebuilds `_layout_rng` as a *fresh* `np.random.RandomState(get_layout_seed())`
//! (`common.py:53-60`). So every layout draws from the start of a fresh stream and the
//! interior is reproducible for **every** call — not only the first after a reset, which is
//! what an earlier version of this comment claimed, and which the reference does not do.
//! The corners draw nothing, so the interior is the stream's first `3 * (n - min(n, 8))`
//! values; one stream serves all three axes of all nodes, in C order (`x`, `y`, `z` per
//! node), because that is how `rng.uniform(-1, 1, (k, 3))` fills its array.

use super::{SCALE, in_space};
use crate::layout::Geometry;
use crate::rng::Mt19937;
use crate::stage::StageError;

/// The eight corners of a cube of half-side 1, in the reference's literal order
/// (`basic.py:91-94`). Transcribed, never derived — see the module doc on why.
///
/// Indexed by node, so node `i` for `i < 8` is `CORNERS[i] * scale`. The order is part of
/// the layout and a change to it is a regression.
pub(super) const CORNERS: [[f64; 3]; 8] = [
    [1.0, 1.0, 1.0],
    [1.0, -1.0, -1.0],
    [-1.0, 1.0, -1.0],
    [-1.0, -1.0, 1.0],
    [-1.0, -1.0, -1.0],
    [-1.0, 1.0, 1.0],
    [1.0, -1.0, 1.0],
    [1.0, 1.0, -1.0],
];

/// `CUBE`'s capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.basic3d.cube";

/// The layout seed every SciGraphs layout is handed: `derive_seed(42, "layout")`
/// (`repro/determinism.py:56-62`, read through `get_layout_seed`, `:124`).
///
/// **This is the reference's number, not a house one.** It is compiled in here so the
/// registered layout is the reference's drawing; the conformance arm passes the same
/// constant as `LAYOUT_SEED` and the two are checked to agree by the pinned coordinates.
/// Changing it moves every hashed snapshot above eight nodes, and nothing below.
const SEED: u32 = 981_798_123;

/// The interior's radius as a fraction of the cube's half-side: `scale * 0.8` against the
/// corners' `scale` (`basic.py:101`). The `0.8` is what makes the scatter **strictly
/// inside** rather than in the shell, and it is the reference's whole reason for using a
/// scaled `uniform` rather than the corners' own scale.
const INTERIOR_RATIO: f64 = 0.8;

/// `_cube_layout(n, scale)` (`basic.py:83-103`) over the node count.
pub(super) fn run(n: u32) -> Result<Geometry, StageError> {
    let (x, y, z) = columns(n);
    Ok(in_space(&x, &y, &z))
}

/// The three `f64` columns at `n`, before narrowing.
pub(super) fn columns(n: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut columns = (
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
        Vec::with_capacity(n as usize),
    );
    // `num_nodes == 1` returns `np.zeros((1, 3))` (`basic.py:88-89`) before the corners are
    // even built: one node is the origin, not a corner. `min(1, 8)` would have put it at
    // `(+scale, +scale, +scale)`, so this branch is load-bearing and not a shortcut.
    if n == 1 {
        columns.0.push(0.0);
        columns.1.push(0.0);
        columns.2.push(0.0);
        return columns;
    }
    let corners = core::cmp::min(n as usize, CORNERS.len());
    for corner in CORNERS.iter().take(corners) {
        columns.0.push(corner[0] * SCALE);
        columns.1.push(corner[1] * SCALE);
        columns.2.push(corner[2] * SCALE);
    }
    interior(&mut columns, n as usize - corners);
    columns
}

/// The scatter of the `remaining` nodes no corner took (`basic.py:99-101`).
///
/// Three [`Mt19937`] draws per node, axis by axis (`x`, then `y`, then `z`), which is the
/// order `rng.uniform(-1, 1, (k, 3))` fills in C order. One stream for all three axes of
/// all nodes, not one per axis: a per-axis stream would reproduce a different drawing, and
/// the difference is invisible in the picture and total in the bytes.
///
/// **The only place this module draws a random number**, and the only place in `basic_3d`
/// that does. `SPHERE` and `HELIX` are closed form and owe no seed.
fn interior(columns: &mut (Vec<f64>, Vec<f64>, Vec<f64>), remaining: usize) {
    let reach = SCALE * INTERIOR_RATIO;
    let mut stream = Mt19937::new(SEED);
    for _ in 0..remaining {
        columns.0.push(uniform(&mut stream) * reach);
        columns.1.push(uniform(&mut stream) * reach);
        columns.2.push(uniform(&mut stream) * reach);
    }
}

/// `rng.uniform(-1.0, 1.0)` (`basic.py:99`), which numpy computes as
/// `low + (high - low) * next_double()` — `(-1.0 + 2.0 * u)` here.
///
/// Written in that operand order, and **in the unit interval**, because the reference then
/// scales the whole array by `(scale * 0.8)`.
///
/// **At `reach = 4.0` the operand order makes no difference, and this is measured.** `4.0` is
/// `2^2`, so the scale by it is exact and `(-1 + 2u) * reach`, `(2*u - 1) * reach` and
/// `-reach + 2*reach*u` agree **bit for bit** over 2e6 draws (0 mismatches, numpy 2.3.3). The
/// test that pins the operand order is therefore checking transcription, not arithmetic — what
/// makes the row pass is the generator and the seed. Two of those three forms are the *same*
/// expression parenthesised differently and can never disagree; only the third can, and it does
/// at a `reach` that is not a power of two (at `reach = 2.96`, `-reach + 2*reach*u` differs from
/// the reference's form on 1.007e6 of 2e6 draws). The reference's form is kept because it is
/// `basic.py:99-101`'s own, and because that is the line a reader checking this port against the
/// reference needs to find.
fn uniform(stream: &mut Mt19937) -> f64 {
    -1.0 + (1.0 - -1.0) * stream.next_f64()
}

#[cfg(test)]
mod tests;
