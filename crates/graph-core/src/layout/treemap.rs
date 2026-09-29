//! Squarified treemap (exact port of d3-hierarchy@3.1.2's `treemap().tile(treemapSquarify)`
//! and `hierarchy.sum`), `layout.treemap.squarified`: `Box` geometry, `O(n log n)`.
//!
//! **Oracle call sequence**, in order, on the [`Hierarchy`]-shaped tree (children
//! ascending dense index; the virtual root's own children are the roots):
//!
//! ```js
//! const root = d3.hierarchy(data, node => node.children)   // children ascending dense index
//!   .sum(node => clampWeight(node.weight))                  // own + children, d3's own order
//!   .sort((a, b) => b.value - a.value);                     // stable: ties keep dense index
//! d3.treemap().tile(d3.treemapSquarify).size([1, 1])(root); // no padding, no round
//! ```
//!
//! `clampWeight` is [`clamp_weight`]: a real node's own value is its topology weight, or
//! [`WEIGHT_EPSILON`] when non-positive or non-finite; the virtual root (never a real
//! node) contributes `0`. `sum` totals own **and** children (`hierarchy/sum.js`), added
//! last-child-first in `f64` so a future oracle matches bit for bit. `sort` is stable, so
//! with children already ascending dense index, sorting only by descending value
//! reproduces d3's ascending-index tie-break with no second key.
//!
//! **Box convention.** d3 tracks `[x0, y0, x1, y1]`; the contract's [`NodeGeometry::Box`]
//! is centre and size — converted once, at the very end, cast to `f32` only then
//! (`to_geometry`).
//!
//! Ponytail: a non-positive or non-finite weight clamps to [`WEIGHT_EPSILON`]
//! (`clamp_weight`) rather than vanishing or handing squarify a zero/NaN value — the
//! oracle applies the identical clamp. Direction: cosmetic under-representation (a
//! hairline, never a wrong containment); escape hatch: fix the weight upstream.
//!
//! **`f32` cast and containment.** `tests`'s invariant sweep checks containment and
//! no-overlap twice: on the exact `f64` boxes (no tolerance needed), and again after each
//! is independently cast to `f32` centre/size and its edges reconstructed. That second
//! pass **does** break exact containment by a few `f32` ULPs even in shallow fixtures —
//! reconstructing an edge from a cast centre and size is a different computation than
//! casting the edge itself. See `tests`'s module doc for the measured bound and the two
//! small, test-only epsilons that absorb it.

use super::Geometry;
use super::hierarchy::Hierarchy;
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::Note;
use rows::{extend_row, node_values, sorted_children};

mod rows;

/// A non-positive or non-finite weight clamps here (module Ponytail).
const WEIGHT_EPSILON: f64 = 1e-6;

/// A real node's clamped weight; the virtual root passes `0.0` directly, never this.
fn clamp_weight(weight: f64) -> f64 {
    if weight.is_finite() && weight > 0.0 {
        weight
    } else {
        WEIGHT_EPSILON
    }
}

/// One box, `[x0, y0, x1, y1]`, `f64` throughout.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Rect {
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
}

impl Rect {
    fn new(x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        Self { x0, y0, x1, y1 }
    }
}

/// Every row's box, dense index including the virtual root's.
#[derive(Debug, Clone)]
struct Boxes(Vec<Rect>);

impl Boxes {
    fn new(rows: usize) -> Self {
        Self(vec![Rect::new(0.0, 0.0, 0.0, 0.0); rows])
    }

    fn rect(&self, v: u32) -> Rect {
        self.0[v as usize]
    }

    fn set(&mut self, v: u32, rect: Rect) {
        self.0[v as usize] = rect;
    }
}

/// The value column and the boxes being written, threaded through squarify together so
/// every helper below still fits the house's 4-parameter cap.
struct Layout<'a> {
    value: &'a [f64],
    boxes: &'a mut Boxes,
}

impl<'a> Layout<'a> {
    fn new(value: &'a [f64], boxes: &'a mut Boxes) -> Self {
        Self { value, boxes }
    }
}

/// `n / d`, or `fallback` when `d` is exactly zero — the one division-by-zero guard
/// `dice`/`slice`/`squarify_children` all need, in one place instead of four.
fn ratio_or(n: f64, d: f64, fallback: f64) -> f64 {
    if d != 0.0 { n / d } else { fallback }
}

/// Splits `row` along x, the full `rect` width (`treemap/dice.js`): every child gets
/// `rect.y0..rect.y1`, and an x-slice proportional to its own value.
fn dice(row: &[u32], rect: Rect, row_value: f64, layout: &mut Layout) {
    let k = ratio_or(rect.x1 - rect.x0, row_value, 0.0);
    let mut cursor = rect.x0;
    for &child in row {
        let x0 = cursor;
        cursor += layout.value[child as usize] * k;
        let r = Rect::new(x0, rect.y0, cursor, rect.y1);
        layout.boxes.set(child, r);
    }
}

/// Splits `row` along y, the full `rect` height (`treemap/slice.js`): every child gets
/// `rect.x0..rect.x1`, and a y-slice proportional to its own value.
fn slice(row: &[u32], rect: Rect, row_value: f64, layout: &mut Layout) {
    let k = ratio_or(rect.y1 - rect.y0, row_value, 0.0);
    let mut cursor = rect.y0;
    for &child in row {
        let y0 = cursor;
        cursor += layout.value[child as usize] * k;
        let r = Rect::new(rect.x0, y0, rect.x1, cursor);
        layout.boxes.set(child, r);
    }
}

/// Tiles `children` (already sorted descending value) into `rect` (`squarifyRatio`): row
/// by row, diced or sliced by whichever of `rect`'s sides is currently longer,
/// `parent_value` (the tiled node's own aggregate, own weight included) the denominator
/// throughout — so a node with positive own weight leaves its children short of the full
/// box, the gap being its own share.
///
/// **The zero-remaining fallback.** d3 writes the row's far edge as
/// `value ? y0 += dy * sumValue / value : y1`: once `value` has cancelled to zero the
/// row's edge is exactly `y1` (or `x1`) and the cursor stays put. `y0 + dy` is not
/// `y1` in `f64` when `x0 != 0`, so the fallback names the far edge itself.
fn squarify_children(children: &[u32], parent_value: f64, rect: Rect, layout: &mut Layout) {
    let n = children.len();
    let (mut x0, mut y0, x1, y1) = (rect.x0, rect.y0, rect.x1, rect.y1);
    let mut remaining = parent_value;
    let mut i0 = 0;
    while i0 < n {
        let (dx, dy) = (x1 - x0, y1 - y0);
        let alpha = (dy / dx).max(dx / dy) / (remaining * golden_ratio());
        let (i1, sum_value) = extend_row(children, i0, alpha, layout.value);
        let row = &children[i0..i1];
        if dx < dy {
            let end_y = if remaining != 0.0 {
                y0 + dy * sum_value / remaining
            } else {
                y1
            };
            dice(row, Rect::new(x0, y0, x1, end_y), sum_value, layout);
            if remaining != 0.0 {
                y0 = end_y;
            }
        } else {
            let end_x = if remaining != 0.0 {
                x0 + dx * sum_value / remaining
            } else {
                x1
            };
            slice(row, Rect::new(x0, y0, end_x, y1), sum_value, layout);
            if remaining != 0.0 {
                x0 = end_x;
            }
        }
        remaining -= sum_value;
        i0 = i1;
    }
}

/// `(1 + sqrt(5)) / 2`, d3's default squarify ratio. `sqrt` is IEEE 754 correctly rounded
/// on every target, unlike `sin`/`cos`/`log`, so this needs no `libm` detour (D1 covers
/// transcendentals, not the one exact op).
fn golden_ratio() -> f64 {
    (1.0 + 5.0_f64.sqrt()) / 2.0
}

/// Every row's exact box; `Boxes::new(0)` only when the topology has no node. Shared by
/// [`run`] and this module's own tests, which check containment on these `f64` boxes and
/// again after each is cast to the contract's `f32` centre/size (`to_geometry`).
fn compute(topology: &Topology, hierarchy: &Hierarchy) -> Boxes {
    let n = topology.node_count();
    let Some(root) = hierarchy.root() else {
        return Boxes::new(0);
    };
    let rows = n + u32::from(hierarchy.virtual_root().is_some());
    let value = node_values(topology, hierarchy);
    let mut boxes = Boxes::new(rows as usize);
    boxes.set(root, Rect::new(0.0, 0.0, 1.0, 1.0));
    let mut layout = Layout::new(&value, &mut boxes);
    for &v in hierarchy.order() {
        let kids = hierarchy.children(v);
        if kids.is_empty() {
            continue;
        }
        let sorted = sorted_children(kids, &value);
        let rect = layout.boxes.rect(v);
        squarify_children(&sorted, value[v as usize], rect, &mut layout);
    }
    boxes
}

/// The `n` real nodes' boxes as the contract's Box convention: centre and size, `f64`
/// throughout, cast to `f32` only in the four pushes below.
fn to_geometry(boxes: &Boxes, n: u32, notes: Vec<Note>) -> Geometry {
    let n = n as usize;
    let mut x = Vec::with_capacity(n);
    let mut y = Vec::with_capacity(n);
    let mut w = Vec::with_capacity(n);
    let mut h = Vec::with_capacity(n);
    for i in 0..n {
        let r = boxes.rect(i as u32);
        x.push(((r.x0 + r.x1) / 2.0) as f32);
        y.push(((r.y0 + r.y1) / 2.0) as f32);
        w.push((r.x1 - r.x0) as f32);
        h.push((r.y1 - r.y0) as f32);
    }
    Geometry {
        nodes: NodeGeometry::Box { x, y, w, h },
        edges: EdgeGeometry::Line,
        notes,
    }
}

/// Runs the squarified treemap at its one, fixed convention: size `[1, 1]`, no padding,
/// no rounding, ratio `phi` — none of it configurable, so there is no `Params` to take.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let hierarchy = Hierarchy::of(topology).map_err(StageError::Capacity)?;
    let boxes = compute(topology, &hierarchy);
    let notes = hierarchy.notes().to_vec();
    Ok(to_geometry(&boxes, topology.node_count(), notes))
}

#[cfg(test)]
mod tests;
