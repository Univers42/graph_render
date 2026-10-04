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
//! **Why the layout sweep is many `#[test]`s.** One serial test over every layout made
//! this the landing gate's long pole: libtest runs tests in parallel, so one long test is
//! one core while nineteen idle. The sweep is cut per registry row, and by seed chunk for
//! the one row that is itself a long pole, over exactly the same `(seed, layout)` pairs
//! with the same assertions and messages. No test spawns a thread, so `RUST_TEST_THREADS`
//! still caps the binary, and the guard `every_registered_layout_has_its_own_sweep` fails
//! if a row, or a seed chunk of a row, is left out. See `docs/measurements/`.

mod geometry_invariants {
    use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
    use graph_core::layout::hierarchy::Hierarchy;
    use graph_core::{
        REFERENCE_DEGREE, Topology, gate_node_count, index_model, layout, registry, seeded_model,
    };
    use std::ops::Range;

    /// Seeds swept: shallow and deep trees, single- and multi-root forests, every note code.
    const SEEDS: u32 = 200;

    /// The chunk marker for a registry row swept whole, in one test, rather than split.
    const WHOLE: u32 = u32::MAX;

    /// Chunks a split registry row is cut into. Only a row that is itself the binary's
    /// long pole is split; splitting all of them would multiply tests for sweeps that
    /// already finish in a fraction of a second.
    const CHUNKS: u32 = 4;

    /// The `f32` centre/size reconstruction tolerance a treemap box's edge can be off by,
    /// restated from `layout::treemap::tests::F32_EDGE_EPSILON` (private to that module).
    const F32_EDGE_EPSILON: f32 = 1e-5;

    fn topology(seed: u32) -> Topology {
        let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
        index_model(&nodes, &edges).expect("the gate model always indexes")
    }

    /// The seed range a chunk marker stands for: `WHOLE` is all of it, any other marker a
    /// `SEEDS.div_ceil(CHUNKS)`-sized slice clamped at `SEEDS`; the guard test checks the tiling.
    fn seed_range(chunk: u32) -> Range<u32> {
        if chunk == WHOLE {
            return 0..SEEDS;
        }
        let per = SEEDS.div_ceil(CHUNKS);
        let start = (chunk * per).min(SEEDS);
        start..(start + per).min(SEEDS)
    }

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

    /// One `#[test]` per `[registry row, chunk, test]` entry below: `[index, id, WHOLE,
    /// name]` sweeps a row whole in one test, `[index, id, chunk, name_sN]` cuts it by seed.
    macro_rules! per_layout_sweep {
        (rows: [ $( [$index:literal, $id:literal, $chunk:tt, $part:ident] ),* $(,)? ]) => {
            /// Every entry, in list order: the guard reads this and nothing else.
            const ROWS: &[(&str, u32)] = &[ $( ($id, $chunk) ),* ];
            $(
                #[test]
                fn $part() {
                    sweep_layout($index, $id, seed_range($chunk));
                }
            )*

            /// The entries must name `registry::LAYOUTS` in order, once each, and every
            /// row's chunks must tile `0..SEEDS` exactly once, so a registry row added
            /// later or a seed chunk dropped here fails instead of going unchecked.
            #[test]
            fn every_registered_layout_has_its_own_sweep() {
                let registered: Vec<&str> = registry::LAYOUTS.iter().map(|c| c.id).collect();
                let mut swept: Vec<&str> = Vec::new();
                for &(id, _) in ROWS {
                    if !swept.contains(&id) {
                        swept.push(id);
                    }
                }
                assert_eq!(swept, registered, "the sweeps must list registry::LAYOUTS in order");
                let seeds: Vec<u32> = (0..SEEDS).collect();
                for &id in &registered {
                    let claimed: Vec<u32> =
                        ROWS.iter().filter_map(|(r, c)| (*r == id).then_some(*c)).collect();
                    let mut covered: Vec<u32> =
                        claimed.iter().flat_map(|&c| seed_range(c)).collect();
                    covered.sort_unstable();
                    assert_eq!(covered, seeds, "{id}: chunks {claimed:?} must tile 0..SEEDS");
                }
            }
        };
    }

    /// One layout's sweep over `seeds`: no `NaN`/`±Inf` (D9), positive finite radii on circles.
    fn sweep_layout(index: usize, id: &str, seeds: Range<u32>) {
        let capability = registry::LAYOUTS[index];
        assert_eq!(
            capability.id, id,
            "the list claims row {index} is {id}, the registry says {}",
            capability.id
        );
        for seed in seeds {
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
        rows: [
            [0, "layout.grid", WHOLE, layout_grid],
            [1, "layout.tree.tidy", WHOLE, layout_tree_tidy],
            [2, "layout.treemap.squarified", WHOLE, layout_treemap_squarified],
            [3, "layout.circular.radial", WHOLE, layout_circular_radial],
            [4, "layout.packing.circle", WHOLE, layout_packing_circle],
            [5, "layout.spectral", WHOLE, layout_spectral],
            [6, "layout.mds.pivot", WHOLE, layout_mds_pivot],
            [7, "layout.force.barnes_hut", WHOLE, layout_force_barnes_hut],
            [8, "layout.forceatlas2", WHOLE, layout_forceatlas2],
            [9, "layout.dag.sugiyama", WHOLE, layout_dag_sugiyama],
            [10, "layout.random", WHOLE, layout_random],
            [11, "layout.circular.ring", WHOLE, layout_circular_ring],
            [12, "layout.spiral", WHOLE, layout_spiral],
            [13, "layout.bipartite", WHOLE, layout_bipartite],
            [14, "layout.force.yifan_hu", WHOLE, layout_force_yifan_hu],
            [15, "layout.force.fruchterman_reingold", WHOLE, layout_force_fruchterman_reingold],
            [16, "layout.force.kamada_kawai", WHOLE, layout_force_kamada_kawai],
            [17, "layout.force.graphopt", WHOLE, layout_force_graphopt],
            // The long pole: 390 s of the sweep's ~1085 s on one core, so the only row cut by seed.
            [18, "layout.force.davidson_harel", 0, layout_force_davidson_harel_s0],
            [18, "layout.force.davidson_harel", 1, layout_force_davidson_harel_s1],
            [18, "layout.force.davidson_harel", 2, layout_force_davidson_harel_s2],
            [18, "layout.force.davidson_harel", 3, layout_force_davidson_harel_s3],
            [19, "layout.force.lgl", WHOLE, layout_force_lgl],
            [20, "layout.force.drl", WHOLE, layout_force_drl],
            [21, "layout.twopi", WHOLE, layout_twopi],
            [22, "layout.packing.osage", WHOLE, layout_packing_osage],
            [23, "layout.force.spring", WHOLE, layout_force_spring],
            [24, "layout.circular.hierarchy", WHOLE, layout_circular_hierarchy],
            [25, "layout.circular.circo", WHOLE, layout_circular_circo],
            [26, "layout.treemap.patchwork", WHOLE, layout_treemap_patchwork],
            [27, "layout.force.neato", WHOLE, layout_force_neato],
            [28, "layout.force.fdp", WHOLE, layout_force_fdp],
            [29, "layout.basic3d.sphere", WHOLE, layout_basic3d_sphere],
            [30, "layout.basic3d.helix", WHOLE, layout_basic3d_helix],
            [31, "layout.basic3d.cube", WHOLE, layout_basic3d_cube],
            [32, "layout.hierarchical3d", WHOLE, layout_hierarchical3d],
            [33, "layout.force.spring3d", WHOLE, layout_force_spring3d],
            [34, "layout.force.sfdp", WHOLE, layout_force_sfdp],
            [35, "layout.forceatlas2.barnes_hut", WHOLE, layout_forceatlas2_barnes_hut],
            [36, "layout.bipartite_3d", WHOLE, layout_bipartite_3d],
            [37, "layout.basic3d.spiral", WHOLE, layout_basic3d_spiral],
            [38, "layout.force.particle_mesh", WHOLE, layout_force_particle_mesh],
            [39, "layout.forceatlas2.forcesim", WHOLE, layout_forceatlas2_forcesim],
            [40, "layout.spectral3d", WHOLE, layout_spectral3d],
            [41, "layout.mds.pivot3d", WHOLE, layout_mds_pivot3d],
        ]
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
