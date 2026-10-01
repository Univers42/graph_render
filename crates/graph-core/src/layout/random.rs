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
use super::coords::point_geometry_with;
use crate::index::Topology;
use crate::stage::StageError;
use crate::synthetic::Mulberry32;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.random";

/// The 3D arm's capability id (`docs/measurements/p12-t4a.md`).
pub const ID_3D: &str = "layout.random.3d";

/// Fixed stream seed; changing it moves every hashed snapshot.
const SEED: u32 = 0x00_5EED;

/// Runs the random layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let (x, y, _) = draw(topology, 2);
    Ok(point_geometry_with(&x, &y, None))
}

/// `layout.random.3d`: the same kernel at three coordinates, so the third draw becomes
/// the z column rather than a second implementation of the same idea.
///
/// SciGraphs `_random_layout` (`basic.py:5-9`) is `rng.rand(n, 3) * scale` — already 3D,
/// the one name here whose reference needs no 2D port at all.
pub fn run_3d(topology: &Topology) -> Result<Geometry, StageError> {
    let (x, y, z) = draw(topology, 3);
    Ok(point_geometry_with(&x, &y, z.as_deref()))
}

/// `dims` draws per node, row-major (`x`, `y`, then `z` at 3), in node order.
///
/// One function, so the two arms cannot drift in *how* they draw — only in how many. At
/// `dims = 2` this is the exact sequence the 2D arm has always consumed, which is what
/// keeps `layout.random`'s bytes where they were: a third draw per node in the 2D arm
/// would move every node after the first, and the hash gate would (correctly) go red.
fn draw(topology: &Topology, dims: usize) -> (Vec<f64>, Vec<f64>, Option<Vec<f64>>) {
    let mut stream = Mulberry32::new(SEED);
    let (mut x, mut y) = (Vec::new(), Vec::new());
    let mut z = (dims >= 3).then(Vec::new);
    for _ in 0..topology.node_count() {
        x.push(stream.next_f64());
        y.push(stream.next_f64());
        if let Some(column) = z.as_mut() {
            column.push(stream.next_f64());
        }
    }
    (x, y, z)
}

#[cfg(test)]
mod tests;
