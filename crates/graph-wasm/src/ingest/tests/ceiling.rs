//! F-16: `ingest::read` refuses a document longer than [`MAX_INGEST_BYTES`] before it
//! parses anything, and a document of exactly that many bytes is not refused for its
//! length. The number is measured and then rounded down to a power of two
//! (`docs/measurements/fix-ingest-scale.md`), so this test is built around it: one buffer of
//! `MAX_INGEST_BYTES + 1` bytes answers both questions.

use crate::errors::Code;
use crate::ingest::{IngestError, MAX_INGEST_BYTES, read};

/// The largest document that built, on the artifact under Node, at the studio's 1M-node
/// scale target: 1M nodes, 7,999,936 edges.
const LARGEST_THAT_BUILT: usize = 1_499_403_588;

/// The first document the same sweep saw trap: 1M nodes, 8,999,919 edges.
const FIRST_THAT_TRAPPED: usize = 1_663_576_802;

/// The ceiling `fix-wasm-ingest` measured with the reader that built a `Value` tree, and the
/// largest document that built *then*. It is here as the floor the new number must clear:
/// a ceiling that refuses a document that used to build replaces nothing.
const PREVIOUS_CEILING: usize = 774_568_785;

/// The ceiling is the largest power of two at or below the largest document that built, and
/// it refuses nothing that built before the reader stopped building a `Value` tree.
#[test]
fn the_ceiling_is_the_largest_power_of_two_at_or_below_the_largest_that_built() {
    let limit = MAX_INGEST_BYTES;
    assert_eq!(limit, 1_073_741_824);
    assert_eq!(limit, 1 << 30);
    // The rule, stated as the rule: a power of two, and at or below both ends of the sweep.
    assert!(limit.is_power_of_two());
    assert!(
        limit <= LARGEST_THAT_BUILT,
        "{limit} is above the largest that built"
    );
    assert!(
        limit < FIRST_THAT_TRAPPED,
        "{limit} is above the first that trapped"
    );
    // And the floor: the number this one replaced, which was itself the largest that built.
    assert!(
        limit > PREVIOUS_CEILING,
        "{limit} refuses a document that used to build"
    );
}

/// One byte past the ceiling is refused by name, with the ceiling's own code; one byte
/// short of it is not refused for its length. The buffer is `MAX_INGEST_BYTES + 1` bytes of
/// JSON whitespace (RFC 8259 allows it before the value), so the bytes are cheap to make and
/// the refusal can only be about the length.
#[test]
fn a_document_one_byte_past_the_ceiling_is_refused_and_one_at_it_is_not() {
    let over = vec![b' '; MAX_INGEST_BYTES + 1];
    let refused = read(&over);
    assert!(
        matches!(
            refused,
            Err(IngestError::TooLarge {
                bytes,
                limit: MAX_INGEST_BYTES
            }) if bytes == MAX_INGEST_BYTES + 1
        ),
        "{:?}",
        refused.err()
    );
    // `MAX_INGEST_BYTES` bytes of spaces then nothing: refused as JSON, never as too long.
    let at = read(&over[..MAX_INGEST_BYTES]);
    assert!(
        !matches!(at, Err(IngestError::TooLarge { .. })),
        "the ceiling refused a document of exactly MAX_INGEST_BYTES bytes: {at:?}"
    );
}

/// The refusal carries its own code, so a host can tell an oversized document from a
/// malformed one; everything else ingest refuses keeps the code it had.
#[test]
fn only_the_ceiling_gets_the_new_code() {
    assert_eq!(
        IngestError::TooLarge { bytes: 1, limit: 0 }.code(),
        Code::IngestTooLarge
    );
    assert_eq!(IngestError::Utf8.code(), Code::IngestInvalid);
    assert_eq!(IngestError::Shape("x".into()).code(), Code::IngestInvalid);
    assert_eq!(IngestError::Capacity.code(), Code::IngestInvalid);
}
