use super::*;

/// The two bundlers are graph-core's own entry points, reached through this table: a row
/// that pointed at another capability's `run` would still compose, and still emit the
/// right geometry kind, and would be wrong.
#[test]
fn the_two_bundler_rows_run_graph_cores_own_entry_points() {
    let (t, g) = pair();
    assert_eq!(
        run_at(&t, &g, "post.bundle.fdeb").geometry,
        graph_core::post::fdeb::run(&t, &g)
            .expect("fdeb runs")
            .geometry
    );
    assert_eq!(
        run_at(&t, &g, "post.bundle.mingle").geometry,
        graph_core::post::mingle::run(&t, &g)
            .expect("mingle runs")
            .geometry
    );
}

/// A capability has to work over whatever a layout emitted, not only over a hand-written
/// `Point` drawing: this is the composability claim, restated over the ABI's own table.
#[test]
fn every_capability_composes_with_every_registered_layout() {
    use graph_core::registry::LAYOUTS;
    let t = topology(
        &["a", "b", "c", "d"],
        &[
            ("e0", "a", "b"),
            ("e1", "b", "c"),
            ("e2", "c", "a"),
            ("e3", "a", "d"),
        ],
    );
    for layout in LAYOUTS {
        let geometry = (layout.run)(&t).unwrap_or_else(|e| panic!("{}: {e}", layout.id));
        for id in IDS {
            let bundled = run_at(&t, &geometry, id);
            assert_eq!(
                bundled.geometry.nodes, geometry.nodes,
                "{id} over {}",
                layout.id
            );
            bundled
                .geometry
                .edges
                .check(t.edge_count())
                .unwrap_or_else(|e| panic!("{id} over {}: {e}", layout.id));
        }
    }
}

/// A `Circle` layout's radius column must survive a post pass untouched, or a caller
/// reading `r` after bundling would draw the wrong circles.
#[test]
fn a_circle_layouts_radius_column_survives_a_post_pass() {
    let t = topology(&["a", "b"], &[("e", "a", "b")]);
    let g = (graph_core::registry::find("layout.packing.circle")
        .expect("registered")
        .run)(&t)
    .expect("packs");
    assert!(matches!(g.nodes, NodeGeometry::Circle { .. }));
    for id in IDS {
        let bundled = run_at(&t, &g, id);
        assert_eq!(bundled.geometry.nodes, g.nodes, "{id}");
    }
}
