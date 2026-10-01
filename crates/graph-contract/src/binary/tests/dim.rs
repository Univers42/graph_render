//! The 3D half of the binary face: the dim byte, the z column on the wire, and the two
//! refusals a z column's own contract demands.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because 3D is a separate
//! concern from the 2D byte layout this file otherwise pins. No 3D layout exists yet, so
//! every 3D snapshot here is built by hand from the parent's 2D helpers.

use super::*;

/// The same snapshot, three-dimensional: a z column and the 0.4 label it requires. No 3D
/// layout exists yet, so this is how a 3D snapshot is built in this crate.
fn parts_3d(nodes: NodeGeometry, edges: EdgeGeometry, z: Vec<f32>) -> SnapshotParts {
    SnapshotParts {
        version: label_for(Dim::D3),
        z: Some(z),
        ..parts(nodes, edges)
    }
}

/// The z column is on the wire where the header says it is: byte 14 is `1`, and the
/// column follows `y`, so a 3D snapshot is longer than the 2D one by exactly `n` words.
#[test]
fn a_3d_snapshot_writes_its_dim_byte_and_one_more_column_than_a_2d_one() {
    for (nodes, z) in [
        (point(), vec![1.0, -2.5]),
        (
            NodeGeometry::Circle {
                x: vec![3.0, -3.0],
                y: vec![0.0, 0.0],
                r: vec![0.0, f32::MAX],
            },
            vec![1.0, 2.0],
        ),
        (
            NodeGeometry::Box {
                x: vec![f32::MIN_POSITIVE; 2],
                y: vec![1e-45; 2],
                w: vec![1.0, 2.0],
                h: vec![0.25, 0.0],
            },
            vec![3.0, 4.0],
        ),
    ] {
        for edges in [
            EdgeGeometry::Line,
            EdgeGeometry::Polyline(Paths {
                offsets: vec![0, 1],
                pts: vec![0.5, 0.25],
            }),
        ] {
            let flat = Snapshot::new(parts(nodes.clone(), edges.clone())).expect("valid");
            let deep = Snapshot::new(parts_3d(nodes.clone(), edges, z.clone())).expect("valid");
            let (flat_bytes, deep_bytes) = (flat.to_bytes(), deep.to_bytes());
            assert_eq!(flat_bytes[14], 0);
            assert_eq!(
                deep_bytes[14], 1,
                "byte 14 is the dim, as the z channel always was"
            );
            assert_eq!(deep_bytes[8], 4, "and the 0.4 label follows from it");
            assert_eq!(flat_bytes[8], 3, "while 2D keeps 0.3");
            assert_eq!(
                deep_bytes.len(),
                flat_bytes.len() + 4 * z.len(),
                "3D is the 2D snapshot plus one word per node"
            );
        }
    }
}

/// The negative control on the wire: a z column that is one value short must be refused by
/// name, not silently padded or truncated. This is the design's own control
/// (`docs/decisions/contract-3d.md` §4.3).
#[test]
fn a_z_column_of_the_wrong_length_is_refused_by_name() {
    let short = SnapshotParts {
        z: Some(vec![1.0]),
        ..parts_3d(point(), EdgeGeometry::Line, vec![1.0, 2.0])
    };
    assert_eq!(
        Snapshot::new(short),
        Err(SnapshotError::Length {
            column: "node.z",
            expected: 2,
            found: 1
        })
    );
    let long = SnapshotParts {
        z: Some(vec![1.0, 2.0, 3.0]),
        ..parts_3d(point(), EdgeGeometry::Line, vec![1.0, 2.0])
    };
    assert_eq!(
        Snapshot::new(long),
        Err(SnapshotError::Length {
            column: "node.z",
            expected: 2,
            found: 3
        })
    );
}

/// A z column under a label that names no dimension is refused rather than written: 0.3
/// cannot express 3D, so a 0.3-labelled snapshot carrying a z would claim a dimension its
/// own version has no word for.
#[test]
fn a_z_column_under_a_label_that_names_no_dimension_is_refused() {
    let relabelled = SnapshotParts {
        version: FormatVersion { major: 0, minor: 3 },
        ..parts_3d(point(), EdgeGeometry::Line, vec![1.0, 2.0])
    };
    assert_eq!(
        Snapshot::new(relabelled),
        Err(SnapshotError::DimUnnameable {
            version: FormatVersion { major: 0, minor: 3 }
        })
    );
}
