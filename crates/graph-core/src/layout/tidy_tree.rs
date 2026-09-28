//! Tidy tree (Reingold–Tilford), `layout.tree.tidy`. An exact port of
//! `d3-hierarchy@3.1.2`'s `tree()` — the Buchheim/Jünger/Leipert/Walker linear-time
//! algorithm, `src/tree.js` at `/home/user/refs/npm/d3-hierarchy-3.1.2/src/tree.js`,
//! carried out in [`walk`] — over the forest [`super::hierarchy::Hierarchy`] repairs (D-H) and
//! hands every layout. No Ponytail: nothing here is a heuristic, an estimate or a
//! fallback, so none is owed.
//!
//! Conventions pinned by this port, all d3 defaults, unchanged:
//! - `size([1, 1])` — the whole tree normalises into the unit square, never `nodeSize`.
//! - `separation(a, b)`: `1.0` when `a` and `b` share a parent, else `2.0`.
//! - every arithmetic step (`firstWalk`/`apportion`/`secondWalk`/the final rescale) runs
//!   in `f64`, in d3's own operation order; only the last write casts to `f32`, so the
//!   two round the same way `Math.fround` would on the JS side.
//!
//! **The oracle arm** (an integration agent's `harness/oracle-layouts.mjs`, not this
//! module) reproduces this input exactly:
//! 1. Repair the forest the same way [`super::hierarchy::Hierarchy`] does (D-H): each node's
//!    parent is its hierarchy edge of the lowest **topology edge index** (among several
//!    candidates), a parent-pointer cycle is broken at its lowest **dense index**, and —
//!    with two or more roots — hang them, ascending, off one synthetic node with no id
//!    and no data.
//! 2. `d3.hierarchy(root, children)` where `root` is the single real root, or that
//!    synthetic virtual-root object when there are ≥2 roots, and `children(node)` gives
//!    exactly [`Hierarchy::children`](super::hierarchy::Hierarchy::children)'s row: kept
//!    children, ascending dense index.
//! 3. `d3.tree()(root)` — every option left at its default (no `.size`, `.nodeSize` or
//!    `.separation` call).
//! 4. Read `node.x`, `node.y` off every real node, in dense-index order, and compare
//!    against this module's `f32` output. The virtual root's own `(x, y)` is real (step
//!    3 folds it into the left/right/bottom normalisation exactly as [`walk`] does) — it
//!    is simply never emitted, on either side.
//!
//! **Edges** are this crate's own convention, not d3's (`tree()` lays out nodes, not
//! edges): every topology edge is a [`EdgeGeometry::Polyline`]. The one kept hierarchy
//! edge that makes a node its parent's child — read from
//! [`Hierarchy::parent_edge`](super::hierarchy::Hierarchy::parent_edge) — gets the elbow, two
//! interior points at mid-depth: `(parent.x, ym)`, `(child.x, ym)` where `ym` is the mean
//! of the parent's and child's `y`. Every other edge — non-hierarchy, self-loop, or
//! dropped by the D-H repair — has zero interior points, drawn straight.

use super::Geometry;
use super::hierarchy::Hierarchy;
use crate::index::Topology;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use walk::Walk;

mod walk;

/// Runs the tidy tree over `topology`'s repaired hierarchy, at d3's defaults. Refused
/// only when the hierarchy repair does not fit the `u32` index space.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    let hierarchy = Hierarchy::of(topology).map_err(StageError::Capacity)?;
    let notes = hierarchy.notes().to_vec();
    let Some(root) = hierarchy.root() else {
        return Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: Vec::new(),
                y: Vec::new(),
            },
            edges: EdgeGeometry::Polyline(Paths::default()),
            notes,
        });
    };
    let mut walk = Walk::new(&hierarchy);
    walk.layout(root);
    let n = topology.node_count();
    let x = (0..n).map(|v| walk.st.x[v as usize] as f32).collect();
    let y = (0..n).map(|v| walk.st.y[v as usize] as f32).collect();
    let edges = build_edges(topology, &hierarchy, &walk.st);
    Ok(Geometry {
        nodes: NodeGeometry::Point { x, y },
        edges,
        notes,
    })
}

/// Every topology edge as a [`EdgeGeometry::Polyline`]: the kept tree edges get the
/// mid-depth elbow, every other edge is drawn straight.
fn build_edges(t: &Topology, h: &Hierarchy, st: &walk::State) -> EdgeGeometry {
    let m = t.edge_count();
    let mut kept = vec![false; m as usize];
    for v in 0..t.node_count() {
        if let Some(e) = h.parent_edge(v) {
            kept[e as usize] = true;
        }
    }
    let mut offsets = Vec::with_capacity(m as usize + 1);
    let mut pts = Vec::new();
    offsets.push(0u32);
    for e in 0..m {
        if kept[e as usize] {
            let v = t.child(e);
            let p = h.parent(v).expect("a kept edge's child has a kept parent");
            let (px, cx) = (st.x[p as usize] as f32, st.x[v as usize] as f32);
            let ym = ((st.y[p as usize] + st.y[v as usize]) / 2.0) as f32;
            pts.extend_from_slice(&[px, ym, cx, ym]);
        }
        offsets.push((pts.len() / 2) as u32);
    }
    EdgeGeometry::Polyline(Paths { offsets, pts })
}

#[cfg(test)]
mod tests;
