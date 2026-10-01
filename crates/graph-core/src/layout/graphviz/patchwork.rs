//! Graphviz 16.1.0's `patchwork`: a squarified treemap over the cluster tree, every node a
//! square whose area comes from its `area` attribute.
//!
//! Reference: `lib/patchwork/patchwork.c` and `lib/patchwork/tree_map.c` of the pinned
//! Graphviz 16.1.0 release, read as an algorithm reference and reimplemented (never
//! translated, never linked — Graphviz is EPL-1.0 and `docs/decisions/graphviz-oracle.md`
//! records that this is an independent implementation agreeing on output, not a copy).
//!
//! **The shape of the thing.** A flat graph with no clusters is one squarified field of
//! equal squares: the drawing is a square of side `sqrt(1000 * n)` points with the nodes
//! packed into it, so the layout says nothing about the edges. The motor emits them as
//! lines between the node centres and the registry's `edges` field says so. `squarify`
//! holds the fill; this module is the field it fills.
//!
//! **Three reference behaviours are reproduced deliberately, and each is a place a
//! reimplementation silently diverges:**
//!
//! 1. **Every node's area is scaled by 1000.** The reference multiplies the `area`
//!    attribute (default 1) by a constant "so that 1 is a reasonable default size", and it
//!    lays out in *points*, so an unscaled port produces a drawing 1000 times too small.
//! 2. **The root square is `sqrt(total + 0.1)`, and the fill rectangle inside it is
//!    `sqrt(total)` — not the same rectangle.** The reference opens with
//!    `r = {0, 0, sqrt(total + 0.1)}` and then insets the field by the margin term
//!    `m = (h + w - sqrt((h-w)² + 4·child_area)) / 2`, which for a square root of side
//!    `sqrt(total + 0.1)` is exactly `sqrt(total)`. The `+ 0.1` is a guard against a
//!    degenerate square and cancels; the field a node lands in has area `1000·n`, and the
//!    closed answers in `tests.rs` are that square's exact multiples.
//! 3. **`-Gstart` is inert here, measured not assumed.** Unlike the force engines this one
//!    draws no random numbers: `docs/measurements/p13-gv1-patchwork.md` records the same
//!    fixture hashing identically at `-Gstart` 1, 7 and 99 over all 1000 seeds. So there is
//!    no seed to match and no chaos to blame for a gap — a difference from Graphviz is an
//!    algorithmic difference or nothing.
//!
//! **Edges are not laid out.** The reference reads none: `patchworkinit.c:115-118` says so
//! in its own words, and the drawing is a treemap. A node's position is a function of its
//! area and of the dense node order alone.
//!
//! Determinism: `squarify` walks areas in index order and writes tile `i` at index `i`, so
//! the output order is the dense node order and nothing iterates a hash or compares
//! floating-point keys (`prompt.md` §6 D1-D10). The trigonometry is one `sqrt`.
//!
//! Ponytail: **`area` and `inset` are not read.** The reference takes both from the DOT
//! attributes, so a graph whose nodes carry unequal areas, or whose clusters carry an
//! `inset`, tiles differently under Graphviz than under this port — which always uses the
//! default area of 1 for every node and no inset. Failing input: any DOT with a node or
//! cluster `area` set to anything but 1, or a cluster `inset`. Direction: the *tiling* is
//! wrong, the field is still a correctly-filled square, and the drawing is still
//! self-consistent — no node escapes the field and no edge is mis-drawn, so it is the
//! cosmetic class of error. Escape hatch: the motor's `Topology` carries no `area`
//! attribute and no cluster membership at all (`graph-contract`'s node and edge records
//! have no such field), so there is nothing to read even in principle; the differential's
//! fixtures are flat graphs on both arms, which is the only place the two can be compared.
//! Closing this means widening `graph-contract`, not this module.

mod squarify;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::coords::point_geometry;
use crate::stage::StageError;

/// The capability id, and the hash gate's stage name.
pub const ID: &str = "layout.treemap.patchwork";

/// `layout.treemap.patchwork` at Graphviz's defaults: every node's default area, no inset,
/// edges as lines between the centres.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let tiles = squarify::tile(topology.node_count());
    let x: Vec<f64> = tiles.iter().map(|t| t.x).collect();
    let y: Vec<f64> = tiles.iter().map(|t| t.y).collect();
    Ok(point_geometry(&x, &y))
}
