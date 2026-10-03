use super::*;
use crate::geometry::Paths;
use crate::snapshot::{Dim, HEADER_LEN, label_for};
use crate::version::{CURRENT_VERSION, NewerMajor};

mod dim;
mod paths;
mod pinned;
mod repeat;

fn table(column: &'static str, items: &[&str]) -> StringTable {
    StringTable::from_strs(column, items.iter().copied()).expect("fits")
}

fn parts(nodes: NodeGeometry, edges: EdgeGeometry) -> SnapshotParts {
    SnapshotParts {
        // 2D, so the 0.3 label: these are the bytes pinned in `pinned.rs`.
        version: label_for(Dim::D2),
        node_ids: table("node.id", &["a", "bc"]),
        edge_ids: table("edge.id", &["e"]),
        source: vec![0],
        target: vec![1],
        nodes,
        z: None,
        edges,
        notes: Notes::default(),
    }
}

fn point() -> NodeGeometry {
    NodeGeometry::Point {
        x: vec![1.0, -2.5],
        y: vec![0.0, 0.5],
    }
}

/// One snapshot of every node kind with every edge kind.
fn every_kind() -> Vec<Snapshot> {
    let two = |v: f32| vec![v, -v];
    let nodes = [
        point(),
        NodeGeometry::Circle {
            x: two(3.0),
            y: two(-0.0),
            r: vec![0.0, f32::MAX],
        },
        NodeGeometry::Box {
            x: two(f32::MIN_POSITIVE),
            y: two(1e-45),
            w: vec![1.0, 2.0],
            h: vec![0.25, 0.0],
        },
    ];
    let paths = Paths {
        offsets: vec![0, 2],
        pts: vec![0.1, 0.2, -0.3, 1e30],
    };
    let edges = [
        EdgeGeometry::Line,
        EdgeGeometry::Polyline(paths.clone()),
        EdgeGeometry::Curve { degree: 3, paths },
    ];
    let mut all = Vec::new();
    for n in &nodes {
        for e in &edges {
            all.push(Snapshot::new(parts(n.clone(), e.clone())).expect("valid"));
        }
    }
    all
}

#[test]
fn every_kind_round_trips_to_the_same_bytes_and_every_column_is_word_aligned() {
    for snapshot in every_kind() {
        let bytes = snapshot.to_bytes();
        assert_eq!(bytes.len() % 4, 0);
        let back = Snapshot::from_bytes(&bytes).expect("reads back");
        assert_eq!(back, snapshot);
        assert_eq!(back.to_bytes(), bytes);
    }
}

#[test]
fn the_header_is_derived_from_what_the_snapshot_holds() {
    let snapshot = &every_kind()[5];
    let header = snapshot.header();
    assert_eq!(header.version, label_for(Dim::D2));
    assert_eq!(header.dim, Dim::D2);
    assert_eq!(header.node_kind, crate::geometry::NodeGeometryKind::Circle);
    assert_eq!(header.edge_kind, crate::geometry::EdgeGeometryKind::Curve);
    assert_eq!((header.node_count, header.edge_count), (2, 1));
    assert_eq!(header.stage_count, StageCount::ONE);
    assert_eq!(snapshot.parts(), &snapshot.clone().into_parts());
}

#[test]
fn version_refusal_of_a_full_snapshot_one_major_ahead() {
    let mut bytes = every_kind()[4].to_bytes();
    let newer = CURRENT_VERSION.major + 1;
    bytes[4..8].copy_from_slice(&newer.to_le_bytes());
    let found = FormatVersion {
        major: newer,
        minor: CURRENT_VERSION.minor,
    };
    // Patch the minor too: these bytes are labelled 0.3, and a header that names a newer
    // minor is a different snapshot than one that names a newer major.
    bytes[8..12].copy_from_slice(&found.minor.to_le_bytes());
    let refusal = SnapshotError::Header(ReadError::UnsupportedMajor(NewerMajor {
        found,
        known: CURRENT_VERSION,
    }));
    assert_eq!(Snapshot::from_bytes(&bytes), Err(refusal));
    assert!(
        refusal
            .to_string()
            .contains("1.4 is newer than this reader's 0.4")
    );
    let mut built = parts(point(), EdgeGeometry::Line);
    built.version = found;
    assert_eq!(Snapshot::new(built), Err(refusal), "nor can one be built");
}

#[test]
fn version_refusal_spares_a_newer_minor_of_this_major() {
    let mut bytes = every_kind()[0].to_bytes();
    assert_eq!(
        CURRENT_VERSION.minor + 1,
        5,
        "a 0.5 snapshot, notes and all"
    );
    bytes[8..12].copy_from_slice(&(CURRENT_VERSION.minor + 1).to_le_bytes());
    let back = Snapshot::from_bytes(&bytes).expect("a newer minor reads");
    assert_eq!(back.parts().version.minor, CURRENT_VERSION.minor + 1);
    assert_eq!(back.to_bytes(), bytes, "and writes back as it was read");
}

#[test]
fn construction_refuses_repeated_ids_stray_endpoints_and_short_ends() {
    let mut p = parts(point(), EdgeGeometry::Line);
    p.node_ids = table("node.id", &["a", "a"]);
    let repeat = |column| SnapshotError::DuplicateId { column, index: 1 };
    assert_eq!(Snapshot::new(p), Err(repeat("node.id")));
    let mut p = parts(point(), EdgeGeometry::Line);
    (p.edge_ids, p.source, p.target) = (table("edge.id", &["e", "e"]), vec![0, 0], vec![1, 1]);
    assert_eq!(Snapshot::new(p), Err(repeat("edge.id")));
    let mut p = parts(point(), EdgeGeometry::Line);
    p.target = vec![2];
    let stray = SnapshotError::Endpoint {
        column: "edge.target",
        index: 0,
    };
    assert_eq!(Snapshot::new(p), Err(stray));
    let mut p = parts(point(), EdgeGeometry::Line);
    p.source = vec![];
    let short = SnapshotError::Length {
        column: "edge.source",
        expected: 1,
        found: 0,
    };
    assert_eq!(Snapshot::new(p), Err(short));
    let mut p = parts(point(), EdgeGeometry::Line);
    p.source = vec![5];
    let stray = SnapshotError::Endpoint {
        column: "edge.source",
        index: 0,
    };
    assert_eq!(Snapshot::new(p), Err(stray));
}

#[test]
fn construction_checks_the_geometry_against_the_counts() {
    let lone = NodeGeometry::Point {
        x: vec![0.0],
        y: vec![0.0],
    };
    assert!(matches!(
        Snapshot::new(parts(lone, EdgeGeometry::Line)),
        Err(SnapshotError::Length {
            column: "node.x",
            ..
        })
    ));
    let paths = EdgeGeometry::Polyline(Paths::default());
    assert!(matches!(
        Snapshot::new(parts(point(), paths)),
        Err(SnapshotError::Length {
            column: "edge.offsets",
            ..
        })
    ));
}

/// The pinned snapshot's bytes with `patch` applied, read back.
fn decode_patched(patch: impl FnOnce(&mut Vec<u8>)) -> Result<Snapshot, SnapshotError> {
    let mut bytes = Snapshot::new(parts(point(), EdgeGeometry::Line))
        .expect("valid")
        .to_bytes();
    patch(&mut bytes);
    Snapshot::from_bytes(&bytes)
}

#[test]
fn the_decoder_refuses_every_malformed_column() {
    use SnapshotError as E;
    let h = HEADER_LEN as usize;
    let offsets = |column, index| Err(E::Offsets { column, index });
    assert_eq!(decode_patched(|b| b[h] = 1), offsets("node.id", 0));
    assert_eq!(decode_patched(|b| b[h + 4] = 4), offsets("node.id", 2));
    let padding = Err(E::Padding { column: "node.id" });
    assert_eq!(decode_patched(|b| b[h + 15] = 9), padding);
    let utf8 = Err(E::Utf8 {
        column: "node.id",
        index: 0,
    });
    assert_eq!(decode_patched(|b| b[h + 12] = 0xff), utf8);
    let padding = Err(E::Padding { column: "edge.id" });
    assert_eq!(decode_patched(|b| b[h + 27] = 1), padding);
    let endpoint = |column| Err(E::Endpoint { column, index: 0 });
    assert_eq!(decode_patched(|b| b[h + 28] = 7), endpoint("edge.source"));
    assert_eq!(decode_patched(|b| b[h + 32] = 2), endpoint("edge.target"));
    let nan = f32::NAN.to_le_bytes();
    let non_finite = Err(E::NonFinite {
        column: "node.y",
        index: 1,
    });
    assert_eq!(
        decode_patched(|b| b[h + 48..h + 52].copy_from_slice(&nan)),
        non_finite
    );
    let trailing = Err(E::TrailingBytes { count: 1 });
    assert_eq!(decode_patched(|b| b.push(0)), trailing);
}

#[test]
fn every_truncation_is_refused_as_truncated() {
    for snapshot in every_kind() {
        let bytes = snapshot.to_bytes();
        for len in 0..bytes.len() {
            match Snapshot::from_bytes(&bytes[..len]) {
                Err(SnapshotError::Truncated { .. }) => {}
                Err(SnapshotError::Header(ReadError::Truncated { .. })) if len < 28 => {}
                other => panic!("{len} of {} bytes: {other:?}", bytes.len()),
            }
        }
    }
}

#[test]
fn a_header_that_claims_more_than_the_payload_is_refused_without_allocating_it() {
    let mut bytes = every_kind()[0].to_bytes();
    bytes[20..24].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        Snapshot::from_bytes(&bytes),
        Err(SnapshotError::Truncated { column: "node.id" })
    );
}

#[test]
fn a_string_table_is_csr_shaped_and_keeps_empty_strings() {
    let t = table("node.id", &["", "é", "\u{1F680}x", ""]);
    assert_eq!(t.len(), 4);
    assert!(!t.is_empty());
    assert_eq!(t.offsets(), [0, 0, 2, 7, 7]);
    assert_eq!(t.bytes(), "é\u{1F680}x".as_bytes());
    assert_eq!(t.iter().collect::<Vec<_>>(), ["", "é", "\u{1F680}x", ""]);
    assert_eq!((t.get(2), t.get(4)), (Some("\u{1F680}x"), None));
    assert!(StringTable::default().is_empty());
    assert_eq!(StringTable::default().len(), 0);
    assert_eq!(t.first_repeat(), Some(3));
    assert_eq!(table("c", &["a", "b"]).first_repeat(), None);
}

#[test]
fn padding_brings_a_length_to_the_next_word() {
    let pads: Vec<usize> = (0..9).map(padding).collect();
    assert_eq!(pads, [0, 3, 2, 1, 0, 3, 2, 1, 0]);
}
