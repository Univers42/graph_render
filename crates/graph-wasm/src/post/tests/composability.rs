//! Composability tests.

use super::fixtures::*;
use graph_core::registry::LAYOUTS;

#[test]
fn every_capability_composes_with_every_registered_layout() {
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
            assert_eq!(
                bundled.geometry.z, geometry.z,
                "{id} over {}: a post pass may not drop the z column",
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

/// Every capability over a 3D geometry keeps its z column: the wasm adapters are the only
/// place a pass rebuilds a geometry on this side of the crate boundary, so a literal there
/// would silently downgrade a 3D drawing to 0.3.
#[test]
fn a_3d_geometries_z_column_survives_every_capability() {
    let (t, _) = pair();
    let z = vec![0.0, 0.5];
    let g = points3(&[0.0, 10.0], &[0.0, 0.0], &z);
    assert_eq!(
        g.dim(),
        graph_contract::snapshot::Dim::D3,
        "the fixture is 3D"
    );
    for id in IDS {
        assert_eq!(run_at(&t, &g, id).geometry.z, Some(z.clone()), "{id}");
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
