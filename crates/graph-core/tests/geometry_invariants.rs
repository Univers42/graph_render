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
    use graph_contract::geometry::NodeGeometry;
    use graph_core::{
        REFERENCE_DEGREE, Topology, gate_node_count, index_model, registry, seeded_model,
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

    mod hierarchy;

    /// One `#[test]` per `[id, chunk, test]` entry below: `[id, WHOLE, name]` sweeps a row
    /// whole in one test, `[id, chunk, name_sN]` cuts it by seed. Rows are found by id, so
    /// an id inserted into the registry adds one entry here and renumbers nothing.
    macro_rules! per_layout_sweep {
        (rows: [ $( [$id:literal, $chunk:tt, $part:ident] ),* $(,)? ]) => {
            /// Every entry, in list order: the guard reads this and nothing else.
            const ROWS: &[(&str, u32)] = &[ $( ($id, $chunk) ),* ];
            $(
                #[test]
                fn $part() {
                    sweep_layout($id, seed_range($chunk));
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
    fn sweep_layout(id: &str, seeds: Range<u32>) {
        let capability = registry::LAYOUTS
            .iter()
            .find(|c| c.id == id)
            .unwrap_or_else(|| panic!("{id} is not in registry::LAYOUTS"));
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
            ["layout.grid", WHOLE, layout_grid],
            ["layout.tree.tidy", WHOLE, layout_tree_tidy],
            ["layout.treemap.squarified", WHOLE, layout_treemap_squarified],
            ["layout.circular.radial", WHOLE, layout_circular_radial],
            ["layout.packing.circle", WHOLE, layout_packing_circle],
            ["layout.spectral", WHOLE, layout_spectral],
            ["layout.mds.pivot", WHOLE, layout_mds_pivot],
            ["layout.force.barnes_hut", WHOLE, layout_force_barnes_hut],
            ["layout.forceatlas2", WHOLE, layout_forceatlas2],
            ["layout.dag.sugiyama", WHOLE, layout_dag_sugiyama],
            ["layout.random", WHOLE, layout_random],
            ["layout.circular.ring", WHOLE, layout_circular_ring],
            ["layout.spiral", WHOLE, layout_spiral],
            ["layout.bipartite", WHOLE, layout_bipartite],
            ["layout.force.yifan_hu", WHOLE, layout_force_yifan_hu],
            ["layout.force.fruchterman_reingold", WHOLE, layout_force_fruchterman_reingold],
            ["layout.force.kamada_kawai", WHOLE, layout_force_kamada_kawai],
            ["layout.force.graphopt", WHOLE, layout_force_graphopt],
            // The long pole: 390 s of the sweep's ~1085 s on one core, so the only row cut by seed.
            ["layout.force.davidson_harel", 0, layout_force_davidson_harel_s0],
            ["layout.force.davidson_harel", 1, layout_force_davidson_harel_s1],
            ["layout.force.davidson_harel", 2, layout_force_davidson_harel_s2],
            ["layout.force.davidson_harel", 3, layout_force_davidson_harel_s3],
            ["layout.force.lgl", WHOLE, layout_force_lgl],
            ["layout.force.drl", WHOLE, layout_force_drl],
            ["layout.twopi", WHOLE, layout_twopi],
            ["layout.packing.osage", WHOLE, layout_packing_osage],
            ["layout.force.spring", WHOLE, layout_force_spring],
            ["layout.circular.hierarchy", WHOLE, layout_circular_hierarchy],
            ["layout.circular.circo", WHOLE, layout_circular_circo],
            ["layout.treemap.patchwork", WHOLE, layout_treemap_patchwork],
            ["layout.force.neato", WHOLE, layout_force_neato],
            ["layout.force.fdp", WHOLE, layout_force_fdp],
            ["layout.basic3d.sphere", WHOLE, layout_basic3d_sphere],
            ["layout.basic3d.helix", WHOLE, layout_basic3d_helix],
            ["layout.basic3d.cube", WHOLE, layout_basic3d_cube],
            ["layout.hierarchical3d", WHOLE, layout_hierarchical3d],
            ["layout.force.spring3d", WHOLE, layout_force_spring3d],
            ["layout.force.sfdp", WHOLE, layout_force_sfdp],
            ["layout.forceatlas2.barnes_hut", WHOLE, layout_forceatlas2_barnes_hut],
            ["layout.bipartite_3d", WHOLE, layout_bipartite_3d],
            ["layout.basic3d.spiral", WHOLE, layout_basic3d_spiral],
            ["layout.force.particle_mesh", WHOLE, layout_force_particle_mesh],
            ["layout.forceatlas2.forcesim", WHOLE, layout_forceatlas2_forcesim],
            ["layout.spectral3d", WHOLE, layout_spectral3d],
            ["layout.mds.pivot3d", WHOLE, layout_mds_pivot3d],
            // merge-p12-t4b: the five 3D arms, appended after `layout.mds.pivot3d`.
            ["layout.force.yifan_hu.2z", WHOLE, layout_force_yifan_hu_2z],
            ["layout.force.fruchterman_reingold.3d", WHOLE, layout_force_fruchterman_reingold_3d],
            ["layout.force.kamada_kawai.3d", WHOLE, layout_force_kamada_kawai_3d],
            ["layout.force.drl.3d", WHOLE, layout_force_drl_3d],
            ["layout.forceatlas2.3d", WHOLE, layout_forceatlas2_3d],
            ["layout.random.3d", WHOLE, layout_random_3d],
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
