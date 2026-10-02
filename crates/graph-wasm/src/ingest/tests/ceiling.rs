//! F-16: `ingest::read` refuses a document longer than [`MAX_INGEST_BYTES`] before it
//! parses anything, and a document of exactly that many bytes is not refused for its
//! length. The number is measured (`docs/measurements/fix-wasm-ingest.md`), so this test
//! is built around it: one buffer of `MAX_INGEST_BYTES + 1` bytes answers both questions.

use crate::errors::Code;
use crate::ingest::{IngestError, MAX_INGEST_BYTES, read};

/// The ceiling is the largest document that built, 774,568,785 bytes, rounded down to a
/// whole MiB: it must refuse nothing that works, which is the whole reason it exists.
#[test]
fn the_ceiling_is_the_largest_that_built_rounded_down_to_a_whole_mib() {
    let limit = MAX_INGEST_BYTES;
    assert_eq!(limit, 773_849_088);
    assert_eq!(limit % (1 << 20), 0, "rounded down to a whole MiB");
    // Below the largest that built, and by less than the rounding step: no document that
    // built is refused, and the ceiling is that measurement rather than a round guess.
    assert!(limit < 774_568_785 && 774_568_785 - limit < (1 << 20));
}

/// One byte past the ceiling is refused by name, with the ceiling's own code; one byte
/// short of it is not refused for its length. The buffer is 738 MiB of JSON whitespace
/// (RFC 8259 allows it before the value), so the bytes are cheap to make and the refusal
/// can only be about the length.
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
    // 738 MiB of spaces then nothing: refused as JSON, never as too long.
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
