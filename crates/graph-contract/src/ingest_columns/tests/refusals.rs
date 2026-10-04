//! The refusals, one test each. Every one of them takes a *valid* document — the encoder's
//! `full()` — patches exactly one thing, and asserts the exact variant. A test that only
//! checked `is_err` would pass for the wrong reason, which is the failure mode these exist
//! to rule out.

use super::*;

/// The refusal `full()` earns after `patch`, or a panic saying it decoded when it must not
/// have. `ColumnsDoc` borrows the bytes and is not `PartialEq`, so the error is pulled out
/// and compared on its own.
fn refused(patch: impl FnOnce(&mut Encoded)) -> ColumnsError {
    let mut encoded = full().encode();
    patch(&mut encoded);
    decode(&encoded.bytes).expect_err("this document is refused")
}

#[test]
fn a_short_buffer_is_refused() {
    let mut bytes = full().encode().bytes;
    bytes.truncate(bytes.len() - 1);
    assert!(matches!(decode(&bytes), Err(ColumnsError::Length { .. })));
}

#[test]
fn a_long_buffer_is_refused() {
    let mut bytes = full().encode().bytes;
    bytes.push(0);
    assert!(matches!(decode(&bytes), Err(ColumnsError::Length { .. })));
}

#[test]
fn a_buffer_shorter_than_the_header_is_refused_before_it_is_read() {
    assert_eq!(
        decode(&[0u8; 31]).expect_err("a header is 32 bytes"),
        ColumnsError::ShortBuffer { found: 31 }
    );
}

#[test]
fn a_bad_magic_is_refused() {
    let at = 0x1234_5678;
    assert_eq!(
        refused(|e| e.patch_header(0, at)),
        ColumnsError::BadMagic { found: at }
    );
}

#[test]
fn a_bad_version_is_refused() {
    assert_eq!(
        refused(|e| e.patch_header(1, 2)),
        ColumnsError::BadVersion { found: 2 }
    );
}

#[test]
fn a_nonzero_reserved_word_is_refused() {
    assert_eq!(
        refused(|e| e.patch_header(6, 1)),
        ColumnsError::NonZeroReserved { word: 1 }
    );
    assert_eq!(
        refused(|e| e.patch_header(7, 9)),
        ColumnsError::NonZeroReserved { word: 9 }
    );
}

#[test]
fn a_count_of_u32_max_is_refused() {
    for (word, name) in [(2, "node_count"), (3, "edge_count"), (4, "string_count")] {
        let at = u32::MAX;
        assert_eq!(
            refused(move |e| e.patch_header(word, at)),
            ColumnsError::CountTooLarge { word: name }
        );
    }
}

#[test]
fn a_blob_that_is_not_utf8_is_refused() {
    let mut e = full().encode();
    e.bytes[e.marks.blob] = 0xff;
    assert_eq!(
        decode(&e.bytes).expect_err("0xff is not UTF-8"),
        ColumnsError::Utf8 { at: 0 }
    );
}

#[test]
fn an_offset_that_does_not_start_at_zero_is_refused() {
    assert_eq!(
        refused(|e| e.patch_u32(e.marks.offsets, 0, 1)),
        ColumnsError::OffsetOrigin { found: 1 }
    );
}

#[test]
fn a_decreasing_offset_is_refused() {
    // offsets[1] is 3 ("n-0"); moving offsets[2] below it leaves the closing offset, and
    // therefore the blob length, untouched, so this is the decreasing rule and not that one.
    assert_eq!(
        refused(|e| e.patch_u32(e.marks.offsets, 2, 1)),
        ColumnsError::DecreasingOffset { index: 1 }
    );
}

#[test]
fn a_last_offset_that_is_not_the_blob_length_is_refused() {
    let last = full().string_count();
    let blob_len = full().blob_len();
    assert_eq!(
        refused(move |e| e.patch_u32(e.marks.offsets, last, blob_len - 1)),
        ColumnsError::OffsetEnd {
            found: blob_len - 1,
            blob_len,
        }
    );
}

#[test]
fn a_slice_that_splits_a_code_point_is_refused() {
    // One two-byte `é` followed by an empty entry: the blob is valid UTF-8 and the closing
    // offset still lands on the blob length, but entry 0's range 0..1 is not a `str`. Only
    // `str::get` can say so — a slice-to-`&str` cast would have asserted it.
    let mut doc = Doc::new();
    let icon = doc.s("\u{e9}");
    doc.s("");
    doc.node().icon = icon;
    let mut encoded = doc.encode();
    encoded.patch_u32(encoded.marks.offsets, 1, 1);
    assert_eq!(
        decode(&encoded.bytes).expect_err("entry 0 is half a code point"),
        ColumnsError::SplitCodePoint { index: 0 }
    );
}

#[test]
fn an_offset_past_the_blob_is_refused() {
    assert_eq!(
        refused(|e| e.patch_u32(e.marks.offsets, 1, 9_999)),
        ColumnsError::OffsetOutOfRange { index: 0 }
    );
}

mod values;

#[test]
fn every_refusal_names_its_own_rule() {
    let texts = [
        (
            ColumnsError::ShortBuffer { found: 3 },
            "buffer of 3 bytes is shorter than the header",
        ),
        (
            ColumnsError::BadMagic { found: 1 },
            "magic 0x1 is not 0x31434d47",
        ),
        (ColumnsError::BadVersion { found: 2 }, "version 2 is not 1"),
        (
            ColumnsError::NonZeroReserved { word: 1 },
            "reserved header word 1 is not 0",
        ),
        (
            ColumnsError::CountTooLarge { word: "edge_count" },
            "edge_count is u32::MAX, the absent marker",
        ),
        (
            ColumnsError::SizeOverflow,
            "a section size does not fit u64",
        ),
        (
            ColumnsError::Length {
                expected: 9,
                found: 8,
            },
            "sections sum to 9 bytes, buffer holds 8",
        ),
        (
            ColumnsError::OffsetOrigin { found: 1 },
            "offsets[0] is 1, not 0",
        ),
        (
            ColumnsError::DecreasingOffset { index: 1 },
            "offsets[1] decreases",
        ),
        (
            ColumnsError::OffsetEnd {
                found: 1,
                blob_len: 2,
            },
            "last offset 1 is not blob_len 2",
        ),
        (
            ColumnsError::OffsetOutOfRange { index: 1 },
            "offsets[1] is past the blob",
        ),
        (ColumnsError::Utf8 { at: 0 }, "blob is not UTF-8 at byte 0"),
        (
            ColumnsError::SplitCodePoint { index: 2 },
            "string 2 splits a code point",
        ),
        (
            ColumnsError::NonZeroPadding { at: 4 },
            "pad byte at 4 is not 0",
        ),
        (
            ColumnsError::StringIndex {
                column: "node id",
                row: 1,
            },
            "node id[1] names no string",
        ),
        (
            ColumnsError::RequiredAbsent {
                column: "node id",
                row: 1,
            },
            "node id[1] is u32::MAX in a required column",
        ),
        (
            ColumnsError::EndpointRow {
                column: "edge source",
                row: 1,
            },
            "edge source[1] names a row past the nodes",
        ),
        (
            ColumnsError::NotBoolean {
                column: "node has_note",
                row: 1,
                found: 2,
            },
            "node has_note[1] is 2, not 0 or 1",
        ),
        (
            ColumnsError::NotFinite {
                column: "node weight",
                row: 1,
            },
            "node weight[1] is not finite",
        ),
    ];
    for (error, text) in texts {
        assert_eq!(error.to_string(), text);
    }
}
