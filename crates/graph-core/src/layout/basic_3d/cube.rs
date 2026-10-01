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
//! **The interior scatter does not reproduce the reference's numbers, and this is the
//! written seeding decision.** The reference draws from `_get_layout_rng()`
//! (`common.py:43-52`), which is a **module-level global** `np.random.RandomState` seeded
//! from `get_layout_seed()` on first use. That has two consequences, both of which make a
//! coordinate-for-coordinate port impossible rather than merely hard:
//!
//! 1. **The stream is numpy's Mersenne Twister, not ours.** This port uses the crate's
//!    own [`Mulberry32`] at [`SEED`], the same stream `layout.random` and the force layouts
//!    use (D5: the motor has no global RNG and the same graph must hash the same on every
//!    target). So **no interior coordinate equals SciGraphs' for any seed.**
//! 2. **The reference's stream is stateful across calls.** `_get_layout_rng()` returns the
//!    *same* `RandomState` every time, so a second `_cube_layout` call in one process
//!    continues the stream rather than restarting it, and its interior depends on every
//!    earlier layout that drew from it. There is no "the" interior to compare against;
//!    only the first call after a reset is reproducible at all.
//!
//! **So the oracle compares the corners exactly and the interior statistically**, and the
//! statistic is written into the metadata: interior coordinates uniform on
//! `[-0.8*scale, 0.8*scale]` per axis, mean 0, variance `(1.6*scale)^2/12`. That is the
//! same answer `registry/closed_form.rs:28-30` gives for `layout.random`, and the same
//! reason. What *is* compared exactly is the part that is actually determined: the eight
//! corners, the `min(n, 8)` split, the single-node origin, and the `0.8` interior radius
//! against the cube's `1.0` — i.e. that the interior is **strictly inside** the shell,
//! which is the property the layout exists to draw.

use super::{SCALE, in_space};
use crate::layout::Geometry;
use crate::stage::StageError;
use crate::synthetic::Mulberry32;

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

/// Fixed stream seed for the interior scatter; changing it moves every hashed snapshot
/// above eight nodes, and nothing below.
const SEED: u32 = 0x00_C0BE;

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
/// Three `Mulberry32` draws per node, axis by axis (`x`, then `y`, then `z`), which is the
/// order `rng.uniform(-1, 1, (k, 3))` fills in C order. One stream for all three axes of
/// all nodes, not one per axis: a per-axis stream would reproduce a different drawing, and
/// the difference is invisible in the picture and total in the bytes.
///
/// **The only place this module draws a random number**, and the only place in `basic_3d`
/// that does. `SPHERE` and `HELIX` are closed form and owe no seed.
fn interior(columns: &mut (Vec<f64>, Vec<f64>, Vec<f64>), remaining: usize) {
    let reach = SCALE * INTERIOR_RATIO;
    let mut stream = Mulberry32::new(SEED);
    for _ in 0..remaining {
        columns.0.push(uniform(&mut stream, reach));
        columns.1.push(uniform(&mut stream, reach));
        columns.2.push(uniform(&mut stream, reach));
    }
}

/// `rng.uniform(-reach, reach)` (`basic.py:101`), which numpy computes as
/// `low + (high - low) * next_double()`.
///
/// Written in that operand order rather than as `(2*u - 1) * reach`: they differ in the
/// last bit, and the reference's form is the one whose distribution `harness` measures.
fn uniform(stream: &mut Mulberry32, reach: f64) -> f64 {
    -reach + (reach - -reach) * stream.next_f64()
}

#[cfg(test)]
mod tests;
