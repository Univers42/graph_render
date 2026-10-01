//! Circular / radial hierarchy, `layout.circular.radial`: concentric rings by BFS depth
//! over the repaired hierarchy ([`super::hierarchy::Hierarchy`], D-H), `Point` geometry,
//! `O(n)`. Oracle: hand (`docs/decisions/circular-conventions.md`), read alongside
//! SciGraphs' `_circular_hierarchy_layout`,
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:693-732`.
//!
//! **Three ring layouts live under this one directory, and they are three different
//! functions.** `layout.circular.radial` is this module: a radial over the *repaired*
//! tree, radius linear in the ring number. `layout.circular.hierarchy`
//! ([`hierarchy`]) is SciGraphs' own `CIRCULAR_HIERARCHY` closed form, radius
//! `max(level, 0.35) * scale / max(2, max_level)` over SciGraphs' own levels.
//! `layout.circular.ring` ([`ring`]) draws no structure at all: every node on one circle
//! in dense-index order. The words overlap; the questions do not.
//!
//! **Ring.** Node `v`'s ring is [`Hierarchy::depth`] directly: a lone real root sits at
//! ring 0 (dead centre); with two or more roots the virtual root — never emitted — takes
//! ring 0 and the real roots land on ring 1, because `depth` is already BFS distance from
//! the tree's own root, virtual or real (D-H). No further `roots >= 2 ? 1 : 0` offset is
//! needed on top of it: `depth` already is that value.
//!
//! **Angle**, within one ring of `k` nodes: ascending dense index gets slot `i = 0..k`,
//! `angle = i * 2*pi/k` — start angle `0` (the positive x axis), increasing as `i` grows.
//! Which way that turns on screen is the renderer's call, same as `y` in `layout/grid.rs`.
//! [`Hierarchy::order`]'s BFS visitation never enters this: a node's slot comes from its
//! own dense index among same-ring nodes, per P3_SPEC's ring rule, not from traversal
//! order.
//!
//! **Radius**: `ring * RING_SPACING`, linear in the ring number and independent of node
//! count — unlike SciGraphs, which normalises every ring into one fixed `scale` by
//! dividing by `max_level`. See `docs/decisions/circular-conventions.md` for the full list
//! of departures this hand oracle takes from SciGraphs' conventions.
//!
//! Edges are this crate's own convention, not SciGraphs' (which draws no edges): every
//! topology edge is a straight [`EdgeGeometry::Line`], endpoints from node geometry.
//!
//! Ponytail: the radius step and the start angle are conventions pinned by this module,
//! not a computation with one right answer (`docs/decisions/circular-conventions.md`).
//! Failing input: a ring holding many nodes at a small radius (a shallow, bushy tree)
//! crowds them close together — arc length per node shrinks as the ring's population
//! grows, its radius held fixed by depth alone. Direction: cosmetic, never wrong — every
//! node keeps its own ring and a distinct slot, so no two real nodes ever collide.
//! Escape hatch: a variant that inflates the radius by ring population, under its own id.

pub mod hierarchy;
pub mod ring;

use super::Geometry;
use super::hierarchy::Hierarchy;
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The layout's capability id, which is also its hash-gate stage.
///
/// Not a `Stage::ID`: this module takes no `Params` (`treemap.rs` and `tidy_tree.rs` pin
/// their conventions the same way), and `Stage` requires a `Params: Default`. The id lives
/// here instead, the one place that names this layout, and `crate::registry::LAYOUTS` and
/// graph-cli's `hashgate` knobs take it from here rather than restating it.
pub const ID: &str = "layout.circular.radial";

/// Distance between adjacent rings; ring 0 is the centre regardless (module Ponytail).
/// Fixed by this module's convention, not a `Params`: `treemap.rs` and `tidy_tree.rs`
/// pin their own conventions the same way.
const RING_SPACING: f64 = 1.0;

/// Runs the circular/radial layout over `topology`'s repaired hierarchy. Refused only
/// when the hierarchy repair does not fit the `u32` index space.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let hierarchy = Hierarchy::of(topology).map_err(StageError::Capacity)?;
    let notes = hierarchy.notes().to_vec();
    let rings: Vec<u32> = (0..topology.node_count())
        .map(|v| hierarchy.depth(v))
        .collect();
    let (x, y) = positions(&rings);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Line,
        notes,
    ))
}

/// Every real node's `(x, y)`, `f64` throughout bar the final cast: ring from
/// [`Hierarchy::depth`] (already computed, one per node, ascending dense index — `rings`
/// is indexed by dense index already), angular slot ascending dense index within the
/// ring.
fn positions(rings: &[u32]) -> (Vec<f32>, Vec<f32>) {
    let counts = ring_counts(rings);
    let mut seen = vec![0u32; counts.len()];
    let mut x = Vec::with_capacity(rings.len());
    let mut y = Vec::with_capacity(rings.len());
    for &ring in rings {
        let slot = seen[ring as usize];
        seen[ring as usize] += 1;
        let (px, py) = point(ring, slot, counts[ring as usize]);
        x.push(px as f32);
        y.push(py as f32);
    }
    (x, y)
}

/// How many real nodes share each ring, indexed by ring number up to the deepest one any
/// node holds; empty when `rings` is.
fn ring_counts(rings: &[u32]) -> Vec<u32> {
    let width = rings
        .iter()
        .max()
        .map_or(0, |&deepest| deepest as usize + 1);
    let mut counts = vec![0u32; width];
    for &ring in rings {
        counts[ring as usize] += 1;
    }
    counts
}

/// One node's centre: radius grows linearly with `ring`; angle is its ascending `slot`
/// (`0..count`) spaced evenly around the ring, starting at zero. `count` is always the
/// number of nodes actually sharing `ring`, so it is never zero here.
fn point(ring: u32, slot: u32, count: u32) -> (f64, f64) {
    let radius = f64::from(ring) * RING_SPACING;
    let angle = f64::from(slot) * (2.0 * std::f64::consts::PI) / f64::from(count);
    (radius * libm::cos(angle), radius * libm::sin(angle))
}

#[cfg(test)]
mod tests;
