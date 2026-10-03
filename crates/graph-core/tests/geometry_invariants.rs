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
//!
//! **Why the layout sweep is one `#[test]` per registry row.** Sweeping every layout in
//! one serial test made this the landing gate's long pole: libtest runs tests in
//! parallel, so a single long test occupies a single core while every other core idles.
//! Splitting by registry row lets the pool spread the rows over the cores it is allowed,
//! and the `(seed, layout)` pairs, the assertions and the messages are all unchanged.
//! No test spawns a thread, so `RUST_TEST_THREADS` still caps the binary. The one list
//! a new registry row has to be added to is pinned by
//! `every_registered_layout_has_its_own_sweep`, so it cannot go unchecked.

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

    /// The ids the generated per-layout sweeps claim must be `registry::LAYOUTS` in
    /// order, so a row added to the registry later fails this guard instead of slipping
    /// past the sweep unchecked.
    fn assert_registry_order(swept: &[&str]) {
        let registered: Vec<&str> = registry::LAYOUTS.iter().map(|c| c.id).collect();
        assert_eq!(
            swept,
            registered.as_slice(),
            "the per-layout sweeps must list registry::LAYOUTS in order"
        );
    }

    /// One `#[test]` per `registry::LAYOUTS` row over all `SEEDS`, plus the guard test.
    macro_rules! per_layout_sweep {
        ($($index:literal => $id:literal as $test:ident),* $(,)?) => {
            /// The ids the generated sweeps below cover, in this list's order.
            const SWEPT_IDS: &[&str] = &[$($id),*];
            $(
                #[test]
                fn $test() {
                    sweep_layout($index, $id);
                }
            )*
            #[test]
            fn every_registered_layout_has_its_own_sweep() {
                assert_registry_order(SWEPT_IDS);
            }
        };
    }

    /// One layout's whole `0..SEEDS` sweep: no `NaN` or `±Inf` in its output (D9), and
    /// positive finite radii wherever it emits circles.
    fn sweep_layout(index: usize, id: &str) {
        let capability = registry::LAYOUTS[index];
        assert_eq!(
            capability.id, id,
            "the list claims row {index} is {id}, the registry says {}",
            capability.id
        );
        for seed in 0..SEEDS {
            let t = topology(seed);
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

    per_layout_sweep! {
        0 => "layout.grid" as layout_grid,
        1 => "layout.tree.tidy" as layout_tree_tidy,
        2 => "layout.treemap.squarified" as layout_treemap_squarified,
        3 => "layout.circular.radial" as layout_circular_radial,
        4 => "layout.packing.circle" as layout_packing_circle,
        5 => "layout.spectral" as layout_spectral,
        6 => "layout.mds.pivot" as layout_mds_pivot,
        7 => "layout.force.barnes_hut" as layout_force_barnes_hut,
        8 => "layout.forceatlas2" as layout_forceatlas2,
        9 => "layout.dag.sugiyama" as layout_dag_sugiyama,
        10 => "layout.random" as layout_random,
        11 => "layout.circular.ring" as layout_circular_ring,
        12 => "layout.spiral" as layout_spiral,
        13 => "layout.bipartite" as layout_bipartite,
        14 => "layout.force.yifan_hu" as layout_force_yifan_hu,
        15 => "layout.force.fruchterman_reingold" as layout_force_fruchterman_reingold,
        16 => "layout.force.kamada_kawai" as layout_force_kamada_kawai,
        17 => "layout.force.graphopt" as layout_force_graphopt,
        18 => "layout.force.davidson_harel" as layout_force_davidson_harel,
        19 => "layout.force.lgl" as layout_force_lgl,
        20 => "layout.force.drl" as layout_force_drl,
        21 => "layout.twopi" as layout_twopi,
        22 => "layout.packing.osage" as layout_packing_osage,
        23 => "layout.force.spring" as layout_force_spring,
        24 => "layout.circular.hierarchy" as layout_circular_hierarchy,
        25 => "layout.circular.circo" as layout_circular_circo,
        26 => "layout.treemap.patchwork" as layout_treemap_patchwork,
        27 => "layout.force.neato" as layout_force_neato,
        28 => "layout.force.fdp" as layout_force_fdp,
        29 => "layout.basic3d.sphere" as layout_basic3d_sphere,
        30 => "layout.basic3d.helix" as layout_basic3d_helix,
        31 => "layout.basic3d.cube" as layout_basic3d_cube,
        32 => "layout.hierarchical3d" as layout_hierarchical3d,
        33 => "layout.force.spring3d" as layout_force_spring3d,
        34 => "layout.force.sfdp" as layout_force_sfdp,
        35 => "layout.forceatlas2.barnes_hut" as layout_forceatlas2_barnes_hut,
        36 => "layout.bipartite_3d" as layout_bipartite_3d,
        37 => "layout.basic3d.spiral" as layout_basic3d_spiral,
        38 => "layout.force.particle_mesh" as layout_force_particle_mesh,
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
