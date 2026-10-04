//! `layout.random`: nodes scattered uniformly in the unit square, edges ignored.
//! Reference: networkx 3.6 `random_layout` (`networkx/drawing/layout.py:64`), which is
//! `rng.rand(n, 2)`: row-major, `x` then `y` per node.
//!
//! The registered stream is this crate's `Mulberry32` under a fixed seed, not numpy's
//! Mersenne Twister: [`run`] takes no seed at all — the motor hashes the same graph the
//! same way on every target (D-rules) — and the SciGraphs arm that *does* take one is
//! [`run_seeded`] below. So the *shape* of networkx's answer is reproduced by [`run`],
//! never its numbers.
//!
//! Ponytail: uniform points overlap and cross freely; that is the layout, not a defect.
//! Failing input: any graph read as a drawing. Escape hatch: another layout id.
//!
//! **No compute tier, measured, not assumed** (`docs/measurements/tier-random.md`). The
//! one-line reason this id is absent from the threaded arm's list, `THREADED_STAGES` at
//! `crates/graph-cli/src/hashgate/tiered.rs:26-33`: the
//! layout **is** the stream, so there is nothing to hand a `StepRange` — a threaded arm
//! that recomputed nothing would print "10-way equal" for a stage no arm computed, which
//! is the one claim the gate exists to make false. `GM_MUTATE_NODE_COUNT` is the control
//! that says the stage *is* hashed anyway: more nodes, more draws, every coordinate
//! moves.
//!
//! **`run_seeded` is the SciGraphs arm, and it is not this id's default.** SciGraphs' own
//! `_random_layout` (`basic.py:5-9`) is `np.random.RandomState(get_layout_seed()).rand(n, 3)
//! * scale` — three axes, a scale, and numpy's generator, so none of the three is what
//! [`run`] does. The conformance harness calls [`run_seeded`] with `LAYOUT_SEED`; the
//! registered default stays on `Mulberry32` and its hash-gate record stands. Adding a
//! seeded entry point rather than moving the default is the rule for every row here
//! (`sfdp::run_seeded` is the same shape).

use super::Geometry;
use super::basic_3d::{SCALE, in_space};
use super::coords::point_geometry;
use crate::index::Topology;
use crate::rng::Mt19937;
use crate::stage::StageError;
use crate::synthetic::Mulberry32;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.random";

/// The 3-D arm's capability id, and the one that SciGraphs' own `_random_layout` is
/// (`docs/decisions/3d-ids.md`, `docs/measurements/p12-3d-oracles.md`).
pub const ID_3D: &str = "layout.random.3d";

/// Fixed stream seed; changing it moves every hashed snapshot.
const SEED: u32 = 0x00_5EED;

/// Runs the random layout; never refuses.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let (x, y, _) = draw(topology, 2);
    Ok(point_geometry(&x, &y))
}

/// `layout.random.3d`: the same kernel at three coordinates, so the third draw becomes
/// the z column rather than a second implementation of the same idea.
///
/// SciGraphs `_random_layout` (`basic.py:5-9`) is `rng.rand(n, 3) * scale` — already 3D,
/// the one name here whose reference needs no 2-D port at all. What is *not* reproduced is
/// the numbers: the stream here is the crate's `Mulberry32` at `SEED`, not numpy's
/// Mersenne Twister off `get_layout_seed()`, so the arm's oracle is the DISTRIBUTION (per
/// axis, mean 1/2 and variance 1/12) and never a coordinate. See [`run_seeded`] for the
/// arm that does compare coordinates, at an explicit seed.
pub fn run_3d(topology: &Topology) -> Result<Geometry, StageError> {
    let (x, y, z) = draw(topology, 3);
    let z = z.expect("dims = 3 draws a z column");
    Ok(in_space(&x, &y, &z))
}

/// `dims` draws per node, row-major (`x`, `y`, then `z` at 3), in node order.
///
/// One function, so the two arms cannot drift in *how* they draw — only in how many. At
/// `dims = 2` this is the exact sequence the 2-D arm has always consumed, which is what
/// keeps `layout.random`'s bytes where they were: a third draw per node in the 2-D arm
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

/// `_random_layout(num_nodes, scale, seed)` (`basic.py:5-9`) at an explicit `seed`:
/// `rng.rand(n, 3) * scale`, row-major, so node `i` takes draws `3i`, `3i+1`, `3i+2` as
/// `x`, `y`, `z`.
///
/// The draw is `rand(n, 3)` and not networkx's `rand(n, 2)`: SciGraphs' own layout is 3D
/// and scaled, and this arm exists to match *SciGraphs*, not networkx. The scale is
/// [`basic_3d::SCALE`](super::basic_3d::SCALE), the dispatcher's `scale = 5.0`
/// (`dispatcher.py:14`), which is the same number `SPHERE` and `CUBE` are drawn at.
pub fn run_seeded(topology: &Topology, seed: u32) -> Result<Geometry, StageError> {
    let n = topology.node_count() as usize;
    let mut stream = Mt19937::new(seed);
    let mut columns = (
        Vec::with_capacity(n),
        Vec::with_capacity(n),
        Vec::with_capacity(n),
    );
    for _ in 0..topology.node_count() {
        columns.0.push(stream.next_f64() * SCALE);
        columns.1.push(stream.next_f64() * SCALE);
        columns.2.push(stream.next_f64() * SCALE);
    }
    Ok(in_space(&columns.0, &columns.1, &columns.2))
}

#[cfg(test)]
mod tests;
