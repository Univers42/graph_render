//! Builds the [`Geometry`](super::super::Geometry) this pipeline produces: `Point` nodes
//! at [`Coords`] and layer Y, `Polyline` edges through each edge's dummy chain.
//!
//! Convention (exact, not a heuristic, like `grid.rs`'s own stated conventions): X is the
//! priority method's own units (`coords.rs`'s `GAP = 1.0`), not centred; Y is
//! `layer × LAYER_SPACING`, growing with the layer, which way is down is the renderer's
//! to decide. A reversed edge's dummy chain was built tail-to-head in acyclic order
//! (`hierarchical.py`'s convention); its interior points are walked back to front so the
//! polyline still runs from the edge's real source to its real target.

use super::acyclic::Acyclic;
use super::coords::Coords;
use super::layering::{Layering, Route};
use graph_contract::geometry::Paths;

/// The spacing between adjacent layers on Y. Matches `coords.rs`'s `GAP` on X, so a
/// dummy's step between layers is the same visual size as a real vertex's step within one.
const LAYER_SPACING: f32 = 1.0;

/// `layering`, `coords` and `acyclic`, bundled so [`edge_paths`] and its helper take one
/// parameter instead of three (≤4 per house style).
pub(crate) struct Routing<'a> {
    pub(crate) layering: &'a Layering,
    pub(crate) coords: &'a Coords,
    pub(crate) acyclic: &'a Acyclic,
}

/// Every real node's `(x, y)`, in node order.
pub(crate) fn node_positions(routing: &Routing, node_count: u32) -> (Vec<f32>, Vec<f32>) {
    let (mut x, mut y) = (Vec::with_capacity(node_count as usize), Vec::new());
    for v in 0..node_count {
        x.push(routing.coords.0[v as usize] as f32);
        y.push(routing.layering.layer_of[v as usize] as f32 * LAYER_SPACING);
    }
    (x, y)
}

/// Every edge's interior points, CSR-shaped: a `Loop`, `Direct` or `Straight` route has
/// none, a `Chain` has one point per dummy.
pub(crate) fn edge_paths(routing: &Routing) -> Paths {
    let m = routing.layering.route.len();
    let mut offsets = Vec::with_capacity(m + 1);
    let mut pts = Vec::new();
    offsets.push(0);
    for e in 0..m as u32 {
        push_route(&mut pts, routing, e);
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

/// Appends edge `e`'s interior points to `pts`, source to target.
fn push_route(pts: &mut Vec<f32>, routing: &Routing, e: u32) {
    let Route::Chain { first, count } = routing.layering.route[e as usize] else {
        return;
    };
    let reversed = routing.acyclic.reversed[e as usize];
    for step in 0..count {
        let d = if reversed {
            first + count - 1 - step
        } else {
            first + step
        };
        pts.push(routing.coords.0[d as usize] as f32);
        pts.push(routing.layering.layer_of[d as usize] as f32 * LAYER_SPACING);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::sugiyama::acyclic::Arcs;
    use crate::layout::sugiyama::layering::{DUMMY_BUDGET, assign_layers};
    use crate::layout::sugiyama::ordering::Ordering;
    use crate::records::build::{edge, node};
    use crate::records::{EdgeRecord, NodeRecord};

    /// `(offsets, pts)` for `nodes`/`edges`, edges in the order given.
    fn paths(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Paths {
        let t = index_model(nodes, edges).expect("fits");
        let acyclic = Acyclic::of(&t);
        let arcs = Arcs::new(&t, &acyclic);
        let layer = assign_layers(&arcs);
        let layering = Layering::build(&arcs, &layer, DUMMY_BUDGET);
        let num_layers = layering.layer_of.iter().copied().max().map_or(0, |m| m + 1);
        let ordering = Ordering::build(&layering, num_layers);
        let coords = Coords::build(&ordering, &layering, t.node_count());
        let routing = Routing {
            layering: &layering,
            coords: &coords,
            acyclic: &acyclic,
        };
        edge_paths(&routing)
    }

    #[test]
    fn a_direct_edge_first_and_last_has_a_zero_width_offset_pair() {
        // Both edges are Direct (span 1): every offset equals the one before it.
        let n = ["a", "b", "c"].map(|id| node(id, ""));
        let e = [edge("ab", "a", "b"), edge("bc", "b", "c")];
        let p = paths(&n, &e);
        assert_eq!(
            p.offsets,
            [0, 0, 0],
            "first and last edge both zero-interior"
        );
        assert!(p.pts.is_empty());
    }

    #[test]
    fn a_max_span_edge_owns_one_point_per_dummy_source_to_target() {
        // a->d spans layers 0..3: 2 dummies, walked in ascending (unreversed) order.
        let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
        let e = [
            edge("ab", "a", "b"),
            edge("bc", "b", "c"),
            edge("cd", "c", "d"),
            edge("ad", "a", "d"),
        ];
        let p = paths(&n, &e);
        assert_eq!(
            p.offsets,
            [0, 0, 0, 0, 2],
            "the last edge owns the only points"
        );
        assert_eq!(p.pts.len(), 4, "2 dummies × (x, y)");
        assert!(
            p.pts[1] < p.pts[3],
            "y rises from the first dummy to the second"
        );
    }

    #[test]
    fn a_reversed_max_span_edge_still_runs_source_to_target() {
        // d->a closes the cycle a->b->c->d->a: it reverses, so its dummy chain (built
        // tail=a to head=d) is walked back to front to still start at d and end at a.
        let n = ["a", "b", "c", "d"].map(|id| node(id, ""));
        let e = [
            edge("ab", "a", "b"),
            edge("bc", "b", "c"),
            edge("cd", "c", "d"),
            edge("da", "d", "a"),
        ];
        let p = paths(&n, &e);
        let (start, end) = (p.offsets[3] as usize, p.offsets[4] as usize);
        let ys: Vec<f32> = p.pts[2 * start..2 * end]
            .chunks(2)
            .map(|xy| xy[1])
            .collect();
        assert!(ys.windows(2).all(|w| w[0] > w[1]), "descends: {ys:?}");
    }
}
