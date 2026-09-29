//! The binary face pinned byte for byte: the worked examples of
//! `docs/contract/binary-layout.md` are these tests.

use super::*;
use crate::notes::{Note, NoteCode, Notes, SNAPSHOT_WIDE};

/// The pinned 84-byte example: nodes `"a"`, `"bc"`, edge `"e"` (a → bc), Point nodes,
/// Line edges, format 0.3, no notes.
fn pinned() -> Vec<u8> {
    #[rustfmt::skip]
    let bytes = [
        &b"GMSN"[..], &[0, 0, 0, 0], &[3, 0, 0, 0], &[0, 0, 0, 0],
        &[1, 0, 0, 0], &[2, 0, 0, 0], &[1, 0, 0, 0],
        &[0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0], b"abc", &[0],
        &[0, 0, 0, 0, 1, 0, 0, 0], b"e", &[0, 0, 0],
        &[0, 0, 0, 0], &[1, 0, 0, 0],
        &[0, 0, 0x80, 0x3f, 0, 0, 0x20, 0xc0],
        &[0, 0, 0, 0, 0, 0, 0, 0x3f],
        &[0, 0, 0, 0],
    ]
    .concat();
    bytes
}

#[test]
fn the_layout_is_pinned_byte_for_byte_for_a_tiny_snapshot() {
    let snapshot = Snapshot::new(parts(point(), EdgeGeometry::Line)).expect("valid");
    let expected = pinned();
    assert_eq!((expected.len(), expected[8]), (84, 3));
    assert_eq!(snapshot.to_bytes(), expected);
    let polyline = Paths {
        offsets: vec![0, 1],
        pts: vec![1.0, 0.5],
    };
    let curve = EdgeGeometry::Curve {
        degree: 2,
        paths: polyline,
    };
    let bytes = Snapshot::new(parts(point(), curve))
        .expect("valid")
        .to_bytes();
    #[rustfmt::skip]
    let tail = [
        2, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0x80, 0x3f, 0, 0, 0, 0x3f,
        0, 0, 0, 0,
    ];
    let mut head = expected[..80].to_vec();
    head[13] = 2;
    assert_eq!(bytes[..80], head[..], "the same, but for the edge tag");
    assert_eq!(bytes[80..], tail, "degree, offsets, pts, then k = 0");
}

#[test]
fn a_snapshot_with_one_note_is_pinned_byte_for_byte() {
    let mut p = parts(point(), EdgeGeometry::Line);
    p.notes = Notes::of(&[Note {
        code: NoteCode::PackingApproximate,
        index: SNAPSHOT_WIDE,
    }]);
    let bytes = Snapshot::new(p).expect("valid").to_bytes();
    let notes: [&[u8]; 3] = [&[1, 0, 0, 0], &[3, 0, 0, 0], &[0xff, 0xff, 0xff, 0xff]];
    let expected = [&pinned()[..80], &notes.concat()].concat();
    assert_eq!((bytes.len(), &bytes), (92, &expected));
    let back = Snapshot::from_bytes(&bytes).expect("reads back");
    assert_eq!(back.to_bytes(), bytes);
}

#[test]
fn a_0_2_snapshot_reads_as_no_notes_and_writes_back_its_80_bytes() {
    let mut old = pinned();
    old.truncate(80);
    old[8] = 2;
    let snapshot = Snapshot::from_bytes(&old).expect("a 0.2 snapshot reads");
    let v0_2 = FormatVersion { major: 0, minor: 2 };
    assert_eq!(snapshot.parts().version, v0_2);
    assert!(snapshot.parts().notes.is_empty());
    assert_eq!(snapshot.to_bytes(), old, "and writes no notes section");
    let mut relabelled = pinned();
    relabelled[8] = 2;
    assert_eq!(
        Snapshot::from_bytes(&relabelled),
        Err(SnapshotError::TrailingBytes { count: 4 }),
        "a 0.2 reader never reads a notes section: a 0.3 one is refused, never misread"
    );
}

#[test]
fn a_0_3_snapshot_missing_its_note_count_is_truncated_not_read_as_none() {
    assert_eq!(
        Snapshot::from_bytes(&pinned()[..80]),
        Err(SnapshotError::Truncated {
            column: "note.count"
        })
    );
}
