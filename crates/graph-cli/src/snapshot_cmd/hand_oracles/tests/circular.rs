//! The circular/radial oracle: the convention it restates, and the node, ring, slot and
//! population its failure message names — worked out here from the hierarchy the layout
//! itself reads, not from the oracle.

use super::*;
use crate::snapshot_cmd::pipeline;

/// `circular`'s own convention, over several sizes, and the node index, ring, slot and
/// population its failure message names.
#[test]
fn the_circular_oracle_catches_a_moved_node_and_names_where_it_sat() {
    for (seed, nodes) in [(0, 4), (1, 30), (7, 60), (13, 200)] {
        let snapshot = pipeline(seed, nodes, "circular.radial")
            .expect("runs")
            .snapshot;
        assert_eq!(circular(seed, nodes, &snapshot), Ok(()), "seed {seed}");
        let v = nodes as usize / 2;
        let (ring, slot, count) = placement(seed, nodes, v);
        let moved = moved(&snapshot, v, (0.5, 0.0));
        let x = match &moved.parts().nodes {
            NodeGeometry::Point { x, .. } => x[v],
            other => panic!("{other:?}"),
        };
        let err = circular(seed, nodes, &moved).expect_err("a moved node");
        let prefix = format!("node {v} (ring {ring} slot {slot}/{count}) at ({x}, ");
        assert!(err.starts_with(&prefix), "{err}");
        assert!(err.contains("the convention puts it at ("), "{err}");
    }
}

/// `(ring, slot, count)` for node `v`, from the hierarchy the layout itself reads: the
/// expected half of the message above, worked out here rather than taken from the oracle.
fn placement(seed: u32, nodes: u32, v: usize) -> (u32, u32, u32) {
    let (records, edges) = seeded_model(seed, nodes, REFERENCE_DEGREE);
    let topology = index_model(&records, &edges).expect("reindexes");
    let hierarchy = Hierarchy::of(&topology).expect("hierarchy");
    let rings: Vec<u32> = (0..topology.node_count())
        .map(|node| hierarchy.depth(node))
        .collect();
    let ring = rings[v];
    let count = rings.iter().filter(|&&r| r == ring).count() as u32;
    let slot = rings[..v].iter().filter(|&&r| r == ring).count() as u32;
    (ring, slot, count)
}
