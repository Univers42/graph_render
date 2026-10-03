//! The point read of `edge.pts` is sized by the last offset, so that offset is checked
//! before it is trusted: an offsets vector that has already broken a rule never gets to
//! choose how much is read.

use super::*;

const OFFSETS: usize = 80;

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().expect("a whole word"))
}

fn put(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// One polyline edge: `edge.offsets` is two words at byte 80, as the pinned layout has
/// it, and its last offset is 1.
fn one_edge() -> Vec<u8> {
    let paths = Paths {
        offsets: vec![0, 1],
        pts: vec![1.0, 0.5],
    };
    let bytes = Snapshot::new(parts(point(), EdgeGeometry::Polyline(paths)))
        .expect("valid")
        .to_bytes();
    assert_eq!(
        bytes.len(),
        100,
        "header, ids, 2 node columns, offsets, pts, note count"
    );
    assert_eq!((word(&bytes, OFFSETS), word(&bytes, OFFSETS + 4)), (0, 1));
    bytes
}

/// Two polyline edges over two nodes, so a decreasing window can sit at index 1 and not
/// only at index 0: `edge.offsets` is three words at byte 92.
fn two_edges() -> Vec<u8> {
    let mut built = parts(
        point(),
        EdgeGeometry::Polyline(Paths {
            offsets: vec![0, 1, 2],
            pts: vec![1.0, 0.5, -1.0, 0.25],
        }),
    );
    built.node_ids = table("node.id", &["a", "b"]);
    built.edge_ids = table("edge.id", &["e", "f"]);
    built.source = vec![0, 1];
    built.target = vec![1, 0];
    let bytes = Snapshot::new(built).expect("valid").to_bytes();
    assert_eq!(
        bytes.len(),
        124,
        "one word of ids, endpoints and offsets more each"
    );
    assert_eq!(
        [word(&bytes, 92), word(&bytes, 96), word(&bytes, 100)],
        [0, 1, 2]
    );
    bytes
}

/// Four bytes of offsets used to choose the point-read length before anything had checked
/// them: offsets starting anywhere but 0, and a last offset that names more points than
/// the payload holds, were both sized into `edge.pts` and refused there as a short
/// payload.
#[test]
fn the_point_read_is_sized_only_after_the_offsets_are_checked() {
    let offsets = |index| SnapshotError::Offsets {
        column: "edge.offsets",
        index,
    };
    let mut starts_nowhere = one_edge();
    put(&mut starts_nowhere, OFFSETS, 1);
    put(&mut starts_nowhere, OFFSETS + 4, 0x8000_0010);
    assert_eq!(
        Snapshot::from_bytes(&starts_nowhere),
        Err(offsets(0)),
        "a head that does not start at 0 is refused before the last one sizes a read"
    );
    let mut decreases = two_edges();
    put(&mut decreases, 96, 100);
    put(&mut decreases, 100, 50);
    assert_eq!(
        Snapshot::from_bytes(&decreases),
        Err(offsets(2)),
        "and so is a window that decreases, named at the offset that breaks it"
    );
}

/// The negative control, and the line between the two faults: the same huge last offset
/// behind a well-formed head is a payload that ends short, not a malformed offsets
/// vector, so it stays [`SnapshotError::Truncated`] — as `pinned.rs` and every truncation
/// in `tests.rs` require.
#[test]
fn a_well_formed_head_over_a_short_payload_is_still_truncated() {
    let mut bytes = one_edge();
    put(&mut bytes, OFFSETS + 4, 0x8000_0010);
    assert_eq!(
        Snapshot::from_bytes(&bytes),
        Err(SnapshotError::Truncated { column: "edge.pts" })
    );
}
