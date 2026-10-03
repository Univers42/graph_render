//! An external, public-API-only check of the geometric invariants the hierarchy layouts
//! promise, over a seed sweep of the gate model. Every test lives inside
//! `mod geometry_invariants`, so `cargo test -p graph-core geometry_invariants` selects
//! them by the (module-qualified) test name, whatever this file itself is called.
//!
//! - **treemap**: every child's box is contained in its parent's, and siblings never
//!   overlap — checked on the `f32` boxes the snapshot actually carries, with the same
//!   `f32`-reconstruction tolerance `layout::treemap::tests` derives and justifies for
//!   an edge cast independently from a box's own (`F32_EDGE_EPSILON` there; a public
//!   integration test cannot reach that private constant, so it is restated here).
//! - **tidy tree**: every `Polyline` edge's offset row is well formed and stays inside
//!   the shared `pts` column.
//! - **every registered layout**: no `NaN` or `±Inf` reaches its output (D9), and circle
//!   packing's radii are always positive and finite.

mod geometry_invariants {
    use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
    use graph_core::layout::hierarchy::Hierarchy;
    use graph_core::{
        REFERENCE_DEGREE, Topology, gate_node_count, index_model, layout, registry, seeded_model,
    };

    /// Seeds swept: enough to draw shallow and deep trees, single- and multi-root forests,
    /// and every note code, without the sweep itself taking more than a moment.
    const SEEDS: u32 = 200;

    /// The `f32` centre/size reconstruction tolerance a treemap box's edge can be off by,
    /// restated from `layout::treemap::tests::F32_EDGE_EPSILON` (private to that module).
    const F32_EDGE_EPSILON: f32 = 1e-5;

    fn topology(seed: u32) -> Topology {
        let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
        index_model(&nodes, &edges).expect("the gate model always indexes")
    }

    /// The four columns a treemap `NodeGeometry::Box` carries, bundled so the checks
    /// below can reconstruct any node's `(x0, y0, x1, y1)` edges without taking four
    /// separate slice parameters each.
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

    #[test]
    fn every_registered_layout_emits_no_nan_or_inf_and_circle_radii_are_positive() {
        for seed in 0..SEEDS {
            let t = topology(seed);
            for capability in registry::LAYOUTS {
                let geometry = (capability.run)(&t)
                    .unwrap_or_else(|e| panic!("seed {seed} {}: {e}", capability.id));
                assert_finite(&geometry.nodes, capability.id, seed);
                if let NodeGeometry::Circle { r, .. } = &geometry.nodes {
                    for (i, &radius) in r.iter().enumerate() {
                        assert!(
                            radius > 0.0 && radius.is_finite(),
                            "seed {seed} {} node {i}: r={radius}",
                            capability.id
                        );
                    }
                }
            }
        }
    }

    fn assert_finite(nodes: &NodeGeometry, id: &str, seed: u32) {
        let columns: [&[f32]; 4] = match nodes {
            NodeGeometry::Point { x, y } => [x, y, &[], &[]],
            NodeGeometry::Circle { x, y, r } => [x, y, r, &[]],
            NodeGeometry::Box { x, y, w, h } => [x, y, w, h],
        };
        for column in columns {
            for &v in column {
                assert!(v.is_finite(), "seed {seed} {id}: non-finite value {v}");
            }
        }
    }
}
