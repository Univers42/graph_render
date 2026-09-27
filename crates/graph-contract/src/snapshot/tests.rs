use super::ReadError::*;
use super::*;
use crate::version::CURRENT_VERSION;

const HEADER: SnapshotHeader = SnapshotHeader {
    version: CURRENT_VERSION,
    node_kind: NodeGeometryKind::Circle,
    edge_kind: EdgeGeometryKind::Polyline,
    stage_count: StageCount::ONE,
    node_count: 7,
    edge_count: 3,
};

fn read(h: SnapshotHeader, patch: impl FnOnce(&mut Vec<u8>)) -> Result<SnapshotHeader, ReadError> {
    let mut bytes = Vec::new();
    h.encode(&mut bytes);
    patch(&mut bytes);
    SnapshotHeader::decode(&bytes)
}

#[test]
fn a_stage_count_can_only_be_one() {
    assert_eq!(StageCount::try_from(1), Ok(StageCount::ONE));
    assert_eq!(StageCount::try_from(0), Err(ReservedStageCount(0)));
    assert_eq!(StageCount::try_from(2), Err(ReservedStageCount(2)));
    assert_eq!((StageCount::ONE.get(), u32::from(StageCount::ONE)), (1, 1));
}

#[test]
fn header_is_exactly_header_len_bytes_and_round_trips() {
    assert_eq!(
        read(HEADER, |b| assert_eq!(b.len(), HEADER_LEN as usize)),
        Ok(HEADER)
    );
}

#[test]
fn version_refusal_of_a_header_one_major_ahead() {
    let mut newer = HEADER;
    newer.version.major = CURRENT_VERSION.major + 1;
    assert_eq!(
        read(newer, |_| ()),
        Err(UnsupportedMajor(NewerMajor {
            found: newer.version,
            known: CURRENT_VERSION
        }))
    );
}

#[test]
fn reader_accepts_a_newer_minor() {
    let mut newer = HEADER;
    newer.version.minor = CURRENT_VERSION.minor + 5;
    assert_eq!(read(newer, |_| ()), Ok(newer));
}

#[test]
fn reader_refuses_reserved_fields_and_short_input() {
    let arc = Err(Geometry(TagError::Reserved(crate::geometry::ARC_TAG)));
    assert_eq!(read(HEADER, |b| b[13] = crate::geometry::ARC_TAG), arc);
    assert_eq!(read(HEADER, |b| b[14] = 1), Err(ReservedZChannel(1)));
    assert_eq!(read(HEADER, |b| b[15] = 9), Err(NonZeroPadding(9)));
    assert_eq!(read(HEADER, |b| b[16] = 2), Err(ReservedStageCount(2)));
    assert_eq!(
        read(HEADER, |b| b.truncate(5)),
        Err(Truncated {
            needed: 28,
            found: 5
        })
    );
    assert_eq!(read(HEADER, |b| b[0] = b'X'), Err(BadMagic));
}

#[test]
fn every_refusal_message_names_the_value_it_refused() {
    let cases = [
        (
            Truncated {
                needed: 28,
                found: 5,
            }
            .to_string(),
            "28 bytes, got 5",
        ),
        (BadMagic.to_string(), "bad magic"),
        (
            UnsupportedMajor(NewerMajor {
                found: FormatVersion { major: 7, minor: 1 },
                known: CURRENT_VERSION,
            })
            .to_string(),
            "format 7.1 is newer than this reader's 0.2",
        ),
        (
            Geometry(TagError::Reserved(4)).to_string(),
            "tag 4 is reserved",
        ),
        (
            Geometry(TagError::Unknown(9)).to_string(),
            "tag 9 is not allocated",
        ),
        (ReservedZChannel(1).to_string(), "z channel 1"),
        (NonZeroPadding(9).to_string(), "byte is 9"),
        (ReservedStageCount(2).to_string(), "stage count 2"),
    ];
    for (message, needle) in cases {
        assert!(message.contains(needle), "{message:?} lacks {needle:?}");
    }
}

#[test]
fn every_snapshot_refusal_names_its_column_and_position() {
    use SnapshotError as E;
    let cases = [
        (E::Header(BadMagic), "bad magic"),
        (
            E::Truncated { column: "node.x" },
            "node.x: the snapshot ends inside it",
        ),
        (
            E::TrailingBytes { count: 3 },
            "3 bytes after the last column",
        ),
        (
            E::Length {
                column: "edge.pts",
                expected: 4,
                found: 2,
            },
            "edge.pts: 2 values, need 4",
        ),
        (
            E::NonFinite {
                column: "node.y",
                index: 5,
            },
            "node.y[5]: NaN or infinite",
        ),
        (
            E::Negative {
                column: "node.r",
                index: 1,
            },
            "node.r[1]: negative",
        ),
        (
            E::Offsets {
                column: "node.id",
                index: 2,
            },
            "node.id[2]: offsets must start at 0",
        ),
        (
            E::Utf8 {
                column: "edge.id",
                index: 0,
            },
            "edge.id[0]: not UTF-8",
        ),
        (
            E::Padding { column: "node.id" },
            "node.id: padding bytes must be 0",
        ),
        (
            E::DuplicateId {
                column: "node.id",
                index: 9,
            },
            "node.id[9]: repeats an earlier id",
        ),
        (
            E::Endpoint {
                column: "edge.target",
                index: 4,
            },
            "edge.target[4]: not a node",
        ),
        (E::CurveDegree, "degree 1 or more"),
        (
            E::Capacity { column: "edge.id" },
            "edge.id: more than a u32",
        ),
    ];
    for (err, needle) in cases {
        let message = err.to_string();
        assert!(message.contains(needle), "{message:?} lacks {needle:?}");
    }
}
