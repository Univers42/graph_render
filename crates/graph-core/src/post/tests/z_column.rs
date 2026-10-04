//! A 3D geometry through every registered capability: the z column comes back, or a
//! node-moving pass refuses it.

use super::*;
use graph_contract::snapshot::Dim;

/// Every registered capability over a 3D geometry: the z column comes back. The matrix above
/// only ever sees 2D inputs, because every layout in the tree is 2D, so without this a pass
/// that dropped z would stay green until the first 3D layout lands.
#[test]
fn a_3d_geometries_z_column_survives_every_registered_capability() {
    let topology = topology();
    let grid = (crate::registry::find("layout.grid")
        .expect("registered")
        .run)(&topology)
    .expect("the grid lays out the test graph")
    .nodes;
    let NodeGeometry::Point { x, y } = grid else {
        panic!("the grid emits Point nodes")
    };
    let z: Vec<f32> = (0..x.len()).map(|i| i as f32 * 0.5).collect();
    let geometry = Geometry::in_space(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Line,
        Vec::new(),
        z.clone(),
    );
    assert_eq!(geometry.dim(), Dim::D3, "the fixture is 3D");
    for cap in &POSTS {
        if cap.meta.moves_nodes {
            // A node-moving pass is refused here rather than run, and that refusal is what
            // keeps the *other* half of this test true. If it ran, it would have to leave z
            // alone while moving x and y under it — the silent downgrade this test exists to
            // catch. `docs/decisions/node-overlap.md` §3 is the decision, and this line is
            // its enforcement: the refusal is asserted, not merely tolerated, so a pass
            // cannot make this green by refusing nothing.
            let err = (cap.run)(&topology, &geometry).expect_err("a node-moving pass refuses 3D");
            assert_eq!(
                err,
                StageError::Param {
                    name: "geometry.z",
                    rule: "must be absent",
                },
                "{}: a node-moving pass must refuse a z column, not half-process it",
                cap.id
            );
            continue;
        }
        let bundled = (cap.run)(&topology, &geometry)
            .unwrap_or_else(|e| panic!("{} over a 3D geometry: {e}", cap.id));
        assert_eq!(bundled.geometry.z, Some(z.clone()), "{}: z dropped", cap.id);
        assert_eq!(
            bundled.geometry.nodes, geometry.nodes,
            "{}: node moved",
            cap.id
        );
    }
}
