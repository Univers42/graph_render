//! The records the `/layout` measurement's fill writes: the smallest the contract admits, and the
//! ids that make a document at `GRAPH_HUB_MAX_DOC_BYTES` hold as many of them as the cap allows.
//!
//! WHY a child module: `memory/upload.rs` is the case and its verdict, this is the arithmetic behind
//! one body. They are split because the reasoning here is about graph-contract's grammar (what a
//! record may leave out) and belongs with the code that writes it, not with the case that counts.

/// The id width, and why it is not one character.
///
/// A document at the cap holds ~745 000 records and ids must be distinct across all of them, so one
/// character (36 of them) is arithmetically impossible. `id` admits `[a-z0-9-]`, so 36^5 = 60 466 176
/// five-character ids cover the count with room to spare, and a **fixed** width is what makes "one
/// record costs the same as every other" a fact rather than an average — a mixed-width document
/// would give the fill a range of per-record costs and the measurement one number to name.
pub const ID_WIDTH: usize = 5;

/// One batch body of `take` records whose ids are `from`, `from + 1`, … in base 36, zero-padded to
/// [`ID_WIDTH`].
///
/// The record is the smallest the contract admits: the four members `read_batch` requires
/// (`collection`, `id`, `updatedAt`, `values`) and one scalar cell. The manifest declares a `title`
/// field and `check_title` runs on the **manifest**, so a record need not carry it
/// (`crates/graph-contract/src/ingest/validate.rs:155`) — which is what lets the record stop at one
/// scalar. `values` is not empty because `Role::Scalar` accepts any JSON (`batch/cells.rs:74`) and a
/// document of no cells is not a document any real workspace holds.
///
/// The collection is written **unqualified** (`task`), which is what a batch spells and what
/// `qualify` (`crates/graph-contract/src/hub/ids.rs:67`) turns into the plugin's own `p0.task` at
/// write time. So one body serves every plugin, and the fill's `record_bytes` — read back off the
/// store rather than computed here — is the qualified length.
///
/// Caveat: `updatedAt` is fixed at 0 rather than incremented per record. D6 says a `u32`, and every
/// record here is a distinct id rather than a distinct version of one, so the value is not what the
/// upload's byte count turns on — but it does mean the fill never exercises a store that has to
/// resolve two versions of one id, which is a different question from this one.
pub fn batch_body(from: u64, take: usize) -> String {
    let mut out = String::with_capacity(take * (ID_WIDTH + 72));
    out.push_str(r#"{"upserts":["#);
    for i in 0..take {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!(
            r#"{{"collection":"task","id":"{}","updatedAt":0,"values":{{"note":"x"}}}}"#,
            base36(from + i as u64, ID_WIDTH)
        ));
    }
    out.push_str(r#"],"deletes":[]}"#);
    out
}

/// `n` in base 36, lowercased and zero-padded to exactly `width` bytes.
///
/// Lowercase and digits only, because `check_record_id` takes what `id` admits and the store's
/// records page orders ids under the `C` collation. The assertion is what makes the width a promise:
/// a count that outgrew `width` digits would otherwise silently repeat an id and rewrite an earlier
/// record, which would shrink the document instead of filling it.
pub fn base36(mut n: u64, width: usize) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![b'0'; width];
    for slot in out.iter_mut().rev() {
        *slot = DIGITS[(n % 36) as usize];
        n /= 36;
    }
    assert_eq!(n, 0, "record {n} needs more than {width} base-36 digits");
    String::from_utf8(out).expect("ASCII digits")
}
