//! `layout.random`: nodes scattered uniformly in the unit square, edges ignored.
//! Reference: networkx 3.6 `random_layout` (`networkx/drawing/layout.py:64`), which is
//! `rng.rand(n, 2)`: row-major, `x` then `y` per node.
//!
//! The stream is this crate's `Mulberry32` under a fixed seed, not numpy's Mersenne
//! Twister: the motor takes no seed, and the same graph must hash the same on every
//! target (D-rules). So the *shape* of networkx's answer is reproduced, never its numbers.
//!
//! Ponytail: uniform points overlap and cross freely; that is the layout, not a defect.
//! Failing input: any graph read as a drawing. Escape hatch: another layout id.
//!
//! **No compute tier, measured, not assumed** (`docs/measurements/tier-random.md`). The
//! one-line reason this id is absent from the threaded arm at `hashgate.rs:169`: the
//! layout **is** the stream, so there is nothing to hand a `StepRange` — a threaded arm
//! that recomputed nothing would print "10-way equal" for a stage no arm computed, which
//! is the one claim the gate exists to make false. `GM_MUTATE_NODE_COUNT` is the control
//! that says the stage *is* hashed anyway: more nodes, more draws, every coordinate
//! moves.

use super::Geometry;
use super::coords::point_geometry;
use crate::index::Topology;
use crate::stage::StageError;
use crate::synthetic::Mulberry32;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.random";

/// Fixed stream seed; changing it moves every hashed snapshot.
const SEED: u32 = 0x00_5EED;

/// Runs the random layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let mut stream = Mulberry32::new(SEED);
    let (x, y): (Vec<f64>, Vec<f64>) = (0..topology.node_count())
        .map(|_| (stream.next_f64(), stream.next_f64()))
        .unzip();
    Ok(point_geometry(&x, &y))
}

#[cfg(test)]
mod tests;
