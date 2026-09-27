use super::ReadError::*;
use super::*;

const HEADER: SnapshotHeader = SnapshotHeader {
    version: CURRENT_VERSION,
    node_kind: NodeGeometryKind::Circle,
    edge_kind: EdgeGeometryKind::Polyline,
    stage_count: 1,
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
fn header_is_exactly_header_len_bytes_and_round_trips() {
    assert_eq!(
        read(HEADER, |b| assert_eq!(b.len(), HEADER_LEN as usize)),
        Ok(HEADER)
    );
}

#[test]
fn reader_refuses_a_major_above_the_one_it_knows() {
    let mut newer = HEADER;
    newer.version.major = CURRENT_VERSION.major + 1;
    let known = CURRENT_VERSION.major;
    assert_eq!(
        read(newer, |_| ()),
        Err(UnsupportedMajor {
            found: known + 1,
            known
        })
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
fn column_writer_refuses_nan_and_infinity_and_writes_nothing() {
    let mut out = Vec::new();
    assert_eq!(
        push_f32_column(&[1.0, f32::NAN], &mut out),
        Err(NonFinite { index: 1 })
    );
    assert_eq!(
        push_f32_column(&[f32::NEG_INFINITY], &mut out),
        Err(NonFinite { index: 0 })
    );
    assert!(out.is_empty());
}

#[test]
fn column_writer_emits_little_endian_bytes() {
    let mut out = Vec::new();
    assert_eq!(push_f32_column(&[1.0, -2.5], &mut out), Ok(()));
    assert_eq!(out, [0, 0, 0x80, 0x3f, 0, 0, 0x20, 0xc0]);
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
            UnsupportedMajor { found: 7, known: 0 }.to_string(),
            "major 7 > known 0",
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
        (NonFinite { index: 3 }.to_string(), "index 3"),
    ];
    for (message, needle) in cases {
        assert!(message.contains(needle), "{message:?} lacks {needle:?}");
    }
}
