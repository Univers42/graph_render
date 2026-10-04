//! The hierarchy layouts' own geometric promises: treemap containment and
//! non-overlap, and tidy tree polyline offsets. A child of `geometry_invariants.rs`.

use super::{SEEDS, topology};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_core::layout;
use graph_core::layout::hierarchy::Hierarchy;

/// The `f32` centre/size reconstruction tolerance a treemap box's edge can be off by,
/// restated from `layout::treemap::tests::F32_EDGE_EPSILON` (private to that module).
const F32_EDGE_EPSILON: f32 = 1e-5;

/// The four columns a treemap `NodeGeometry::Box` carries, bundled so the checks can
/// reconstruct any node's `(x0, y0, x1, y1)` edges without four slice parameters.
struct Boxes<'a> {
    x: &'a [f32],
    y: &'a [f32],
    w: &'a [f32],
    h: &'a [f32],
}

impl Boxes<'_> {
    /// `(x0, y0, x1, y1)` of node `v`'s box, reconstructed from its `f32` centre and size.
    fn rect(&self, v: u32) -> (f32, f32, f32, f32) {
        let v = v as usize;
        let (cx, cy, hw, hh) = (self.x[v], self.y[v], self.w[v] / 2.0, self.h[v] / 2.0);
        (cx - hw, cy - hh, cx + hw, cy + hh)
    }
}

#[test]
fn treemap_boxes_contain_their_children_and_siblings_never_overlap() {
    for seed in 0..SEEDS {
        let t = topology(seed);
        let hierarchy = Hierarchy::of(&t).expect("fits u32");
        let geometry = layout::treemap::run(&t).expect("valid");
        let NodeGeometry::Box { x, y, w, h } = geometry.nodes else {
            panic!("seed {seed}: treemap did not emit Box geometry");
        };
        let boxes = Boxes {
            x: &x,
            y: &y,
            w: &w,
            h: &h,
        };
        for v in 0..t.node_count() {
            let kids = hierarchy.children(v);
            check_containment(seed, v, &boxes, kids);
            check_no_overlap(seed, v, &boxes, kids);
        }
    }
}

fn check_containment(seed: u32, parent_id: u32, boxes: &Boxes, kids: &[u32]) {
    let (px0, py0, px1, py1) = boxes.rect(parent_id);
    for &c in kids {
        let (cx0, cy0, cx1, cy1) = boxes.rect(c);
        assert!(
            cx0 >= px0 - F32_EDGE_EPSILON,
            "seed {seed} node {c}: x0 escapes parent {parent_id}"
        );
        assert!(
            cy0 >= py0 - F32_EDGE_EPSILON,
            "seed {seed} node {c}: y0 escapes parent {parent_id}"
        );
        assert!(
            cx1 <= px1 + F32_EDGE_EPSILON,
            "seed {seed} node {c}: x1 escapes parent {parent_id}"
        );
        assert!(
            cy1 <= py1 + F32_EDGE_EPSILON,
            "seed {seed} node {c}: y1 escapes parent {parent_id}"
        );
    }
}

fn check_no_overlap(seed: u32, parent_id: u32, boxes: &Boxes, kids: &[u32]) {
    for (i, &a) in kids.iter().enumerate() {
        for &b in &kids[i + 1..] {
            let (ax0, ay0, ax1, ay1) = boxes.rect(a);
            let (bx0, by0, bx1, by1) = boxes.rect(b);
            let separate = ax1 <= bx0 + F32_EDGE_EPSILON
                || bx1 <= ax0 + F32_EDGE_EPSILON
                || ay1 <= by0 + F32_EDGE_EPSILON
                || by1 <= ay0 + F32_EDGE_EPSILON;
            assert!(
                separate,
                "seed {seed}: siblings {a} and {b} of {parent_id} overlap"
            );
        }
    }
}

#[test]
fn tidy_tree_polyline_offsets_are_well_formed_and_stay_inside_pts() {
    for seed in 0..SEEDS {
        let t = topology(seed);
        let geometry = layout::tidy_tree::run(&t).expect("valid");
        let EdgeGeometry::Polyline(paths) = geometry.edges else {
            panic!("seed {seed}: tidy tree did not emit Polyline edges");
        };
        assert_eq!(
            paths.offsets.len(),
            t.edge_count() as usize + 1,
            "seed {seed}"
        );
        assert_eq!(paths.offsets[0], 0, "seed {seed}: offsets start at 0");
        for pair in paths.offsets.windows(2) {
            assert!(
                pair[0] <= pair[1],
                "seed {seed}: offsets must never decrease"
            );
        }
        let points = *paths.offsets.last().expect("at least one offset");
        assert_eq!(
            points as usize * 2,
            paths.pts.len(),
            "seed {seed}: pts holds an (x, y) pair for every offset row"
        );
    }
}
