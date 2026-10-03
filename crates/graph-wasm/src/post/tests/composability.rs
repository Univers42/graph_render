//! Composability tests.

use super::fixtures::*;
use graph_core::registry::LAYOUTS;

use super::fixtures::NODE_MOVER;

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
            // A 3D layout is refused by the node mover rather than flattened, so it composes
            // with every layout that is 2D and refuses the ones that are not. The refusal is
            // asserted, not skipped: the point is that it is a refusal.
            if id == NODE_MOVER && geometry.z.is_some() {
                let refused = (crate::post::CAPABILITIES[index_of(id)].run)(&t, &geometry)
                    .expect_err("a node mover must refuse a 3D layout");
                assert!(
                    refused.to_string().contains("geometry.z"),
                    "{id} over {}: the refusal names the column, got {refused}",
                    layout.id
                );
                continue;
            }
            let bundled = run_at(&t, &geometry, id);
            // **The node mover is the one capability that rewrites `x` and `y`.** It is held
            // to the other half of the contract instead — same node kind, same node count, a
            // well-formed edge column — because "the nodes come back exactly as they went in"
            // is exactly what it exists not to do. Every other pass keeps that assertion.
            if id != NODE_MOVER {
                assert_eq!(
                    bundled.geometry.nodes, geometry.nodes,
                    "{id} over {}",
                    layout.id
                );
            } else {
                assert_eq!(
                    graph_core::post::centres(&bundled.geometry.nodes).0.len(),
                    graph_core::post::centres(&geometry.nodes).0.len(),
                    "{id} over {}: one position per node, or the pass invented or lost one",
                    layout.id
                );
                assert!(
                    std::mem::discriminant(&bundled.geometry.nodes)
                        == std::mem::discriminant(&geometry.nodes),
                    "{id} over {}: the node geometry kind is never changed",
                    layout.id
                );
            }
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
///
/// **Except the node mover, which refuses a z column**: it moves `x` and `y` and cannot reach
/// `z`, so a 3D geometry is `StageError::Param` rather than a silently flattened drawing. That
/// is the contract (`docs/decisions/node-overlap.md` §3, devil condition 3), so it is asserted
/// here rather than skipped — an adapter that swallowed the refusal would downgrade the drawing,
/// and an adapter that refused a 3D layout the other passes keep would lie in the other
/// direction.
#[test]
fn a_3d_geometries_z_column_survives_every_capability_but_the_node_movers() {
    let (t, _) = pair();
    let z = vec![0.0, 0.5];
    let g = points3(&[0.0, 10.0], &[0.0, 0.0], &z);
    assert_eq!(
        g.dim(),
        graph_contract::snapshot::Dim::D3,
        "the fixture is 3D"
    );
    for id in IDS {
        if id == NODE_MOVER {
            let refused = (crate::post::CAPABILITIES[index_of(id)].run)(&t, &g)
                .expect_err("a node mover must refuse a z column");
            assert!(
                refused.to_string().contains("geometry.z"),
                "{id}: the refusal names the column, got {refused}"
            );
            continue;
        }
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
