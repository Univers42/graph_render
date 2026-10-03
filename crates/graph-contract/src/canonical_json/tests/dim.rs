//! The 3D half of the JSON face: the `dim` member, the z column, and the negative control
//! that says a z column is compared rather than merely carried.
//!
//! Split out of `tests.rs` by the house's 300-line limit, and because 3D is a separate
//! concern from the 2D text this file otherwise pins. Every 3D snapshot here is built by
//! hand from the parent's 2D helpers: this crate depends on no `graph-core`, so a layout is
//! out of its reach however many the registry holds.

use super::*;

/// The 3D cases are built by hand because this crate cannot run a layout: `spaced` is the
/// whole of what a 3D snapshot owes a reader, one `z` column and the 0.4 label.
#[test]
fn a_3d_snapshot_survives_the_binary_face_byte_for_byte() {
    for (nodes, z) in [
        (point(), vec![0.0, -0.5]),
        (
            NodeGeometry::Circle {
                x: vec![1.0, 2.0],
                y: vec![3.0, 4.0],
                r: vec![0.5, 0.25],
            },
            vec![5.0, 6.0],
        ),
        (
            NodeGeometry::Box {
                x: vec![1.0, 2.0],
                y: vec![3.0, 4.0],
                w: vec![0.5, 0.25],
                h: vec![0.125, 0.0625],
            },
            vec![7.0, 8.0],
        ),
    ] {
        for edges in [
            EdgeGeometry::Line,
            EdgeGeometry::Polyline(Paths {
                offsets: vec![0, 1],
                pts: vec![0.5, 0.25],
            }),
            EdgeGeometry::Curve {
                degree: 2,
                paths: Paths {
                    offsets: vec![0, 1],
                    pts: vec![0.5, 0.25],
                },
            },
        ] {
            let snapshot = Snapshot::new(SnapshotParts {
                version: label_for(Dim::D3),
                z: Some(z.clone()),
                ..snapshot(nodes.clone(), edges).into_parts()
            })
            .expect("a 3D snapshot is valid");
            let bytes = snapshot.to_bytes();
            assert_eq!(bytes[14], 1, "the header says 3D");
            let back = Snapshot::from_bytes(&bytes).expect("a 3D snapshot reads back");
            assert_eq!(back, snapshot);
            assert_eq!(back.to_bytes(), bytes);
            assert_eq!(back.parts().z.as_deref(), Some(z.as_slice()));
        }
    }
}

#[test]
fn a_3d_snapshot_round_trips_byte_exact_through_both_faces() {
    for (nodes, z) in [
        (point(), vec![0.0, -0.5]),
        (
            NodeGeometry::Circle {
                x: vec![1.0, 2.0],
                y: vec![3.0, 4.0],
                r: vec![0.5, 0.25],
            },
            vec![5.0, 6.0],
        ),
        (
            NodeGeometry::Box {
                x: vec![1.0, 2.0],
                y: vec![3.0, 4.0],
                w: vec![0.5, 0.25],
                h: vec![0.125, 0.0625],
            },
            vec![7.0, 8.0],
        ),
    ] {
        for edges in [
            EdgeGeometry::Line,
            EdgeGeometry::Polyline(Paths {
                offsets: vec![0, 1],
                pts: vec![0.5, 0.25],
            }),
            EdgeGeometry::Curve {
                degree: 2,
                paths: Paths {
                    offsets: vec![0, 1],
                    pts: vec![0.5, 0.25],
                },
            },
        ] {
            let s = spaced(nodes.clone(), edges, z.clone());
            let text = to_json(&s);
            let back = from_json(&text).expect("a 3D document reads back");
            assert_eq!(back.to_bytes(), s.to_bytes(), "binary is byte-exact");
            assert_eq!(to_json(&back), text, "and the text is a fixed point");
            assert_eq!(back.parts().dim(), Dim::D3);
        }
    }
}

/// Condition 3 in one sentence: `"dim"` is written iff the label is 0.4 or later, which
/// by the label rule is iff the snapshot is 3D, and it reads as 0 when absent.
#[test]
fn dim_is_written_from_0_4_on_and_reads_as_0_when_absent() {
    let two = snapshot(point(), EdgeGeometry::Line);
    let text = to_json(&two);
    assert!(
        !text.contains(r#""dim""#),
        "a 0.3 snapshot writes no dim at all: {text}"
    );
    assert_eq!(two.parts().version, label_for(Dim::D2));
    let three = spaced(point(), EdgeGeometry::Line, vec![0.0, 1.0]);
    let text = to_json(&three);
    assert!(text.starts_with(r#"{"dim":1,"edges""#), "{text}");
    assert!(
        text.contains(r#""nodes":{"kind":"Point","x":[1,-0],"y":[0.1,"#)
            && text.contains(r#""z":["#),
        "the z column sits with the other node columns: {text}"
    );
    let absent = text.replacen(r#""dim":1,"#, "", 1);
    assert_eq!(
        from_json(&absent),
        Err(JsonError::Shape {
            path: "geometry.nodes.z".into(),
            what: "is not part of a 2D snapshot's shape"
        }),
        "dim absent reads as 0, so a z column is then not part of the shape at all"
    );
    let no_z = text.replacen(r#""z":[0,1]"#, r#""zz":[0,1]"#, 1);
    assert_ne!(no_z, text, "the edit landed");
    assert_eq!(
        from_json(&no_z),
        Err(JsonError::Shape {
            path: "geometry.nodes.z".into(),
            what: "is missing"
        }),
        "renaming z away leaves a dim of 1 with no z, which is refused"
    );
    let no_dim_no_z = absent.replacen(r#","z":[0,1]"#, "", 1);
    assert_ne!(no_dim_no_z, absent, "the edit landed");
    let plain = from_json(&no_dim_no_z).expect("a 2D document with neither dim nor z reads");
    assert_eq!(
        plain.parts().z,
        None,
        "absent dim reads as 0, so there is no z"
    );
    assert_eq!(
        plain.to_bytes()[14],
        0,
        "and byte 14 is 0 again, whatever the document said its version was"
    );
    assert_eq!(
        plain.parts().version,
        label_for(Dim::D3),
        "the version stays what the document declared: 0.4 can express a 2D snapshot, \
         it just does not have to"
    );
    let zero = text.replacen(r#""dim":1"#, r#""dim":0"#, 1);
    assert_eq!(
        from_json(&zero),
        Err(JsonError::Shape {
            path: "geometry.nodes.z".into(),
            what: "is not part of a 2D snapshot's shape"
        }),
        "and dim 0 with a z column is refused rather than the z quietly dropped"
    );
}

/// The negative control for the round trip: a z column that is perturbed in the JSON face
/// must fail the byte comparison, and so must one perturbed in the binary face. The
/// snapshot's hash is taken over these bytes (`docs/contract/binary-layout.md`), so a
/// comparison that holds here is a comparison the hash cannot slip past.
#[test]
fn a_perturbed_z_column_fails_the_round_trip_in_both_directions() {
    let s = spaced(point(), EdgeGeometry::Line, vec![0.0, 1.0]);
    let text = to_json(&s);
    let edited = text.replacen(r#""z":[0,1]"#, r#""z":[0,2]"#, 1);
    assert_ne!(edited, text, "the edit landed");
    let from_text = from_json(&edited).expect("still a valid document");
    assert_ne!(
        from_text.to_bytes(),
        s.to_bytes(),
        "a z that moved is a different snapshot, not a re-spelling of the same one"
    );
    assert_ne!(to_json(&from_text), text);

    // And the same perturbation in the binary face. The z value is one that appears
    // nowhere else in the snapshot, so the word found below is the z and not a
    // coincidental match in a column or in an id's bytes.
    let unique = 0.375f32;
    let other = spaced(point(), EdgeGeometry::Line, vec![0.0, unique]);
    let mut bytes = other.to_bytes();
    let word = unique.to_le_bytes();
    let at = bytes
        .windows(4)
        .rposition(|w| w == word)
        .expect("the snapshot's z holds the unique value");
    assert_eq!(other.parts().z.as_deref(), Some([0.0, unique].as_slice()));
    bytes[at..at + 4].copy_from_slice(&2.0f32.to_le_bytes());
    let from_bytes = Snapshot::from_bytes(&bytes).expect("still readable");
    assert_eq!(
        from_bytes.parts().z.as_deref(),
        Some([0.0, 2.0].as_slice()),
        "the z column is read back where it was written, not dropped or re-derived"
    );
    assert_ne!(
        from_bytes.to_bytes(),
        other.to_bytes(),
        "so a moved z is a different snapshot, and its bytes differ"
    );
    assert_eq!(
        from_bytes.to_bytes(),
        bytes,
        "while the bytes that were read are the bytes that are written back"
    );
}
