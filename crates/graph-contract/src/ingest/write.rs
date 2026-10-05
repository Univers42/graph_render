//! The canonical writer: [`Ingest`] to the document's wire text, and [`JsonValue`] to
//! one JSON value's.
//!
//! Canonical means the same thing here as in `canonical_json` (`prompt.md` §4.1): one
//! text per document — compact, no space anywhere, **keys sorted by bytes at every
//! depth**, arrays in document order, one trailing newline. A `values` map is therefore
//! written sorted by field id whatever order it was read in, which is what makes two
//! adapters' output byte-comparable (H6).

use super::{Collection, Field, Ingest, JsonValue};
use core::fmt::Write;

mod pieces;
mod value;

use value::value as write_value;

pub use pieces::*;

/// The canonical wire text of one document.
///
/// Panics if `doc.version` is not `super::VERSION`: this is a caller contract
/// violation, not a document fault, and `read` refuses the same text — so a document
/// the writer cannot round trip must not be written at all, loudly, at the call that
/// made it.
pub fn to_json(doc: &Ingest) -> String {
    assert!(
        doc.version == super::VERSION,
        "ingest document version {} is not the ingest version {} this contract writes and reads",
        doc.version,
        super::VERSION,
    );
    let mut out = String::from(DOC_HEAD);
    push_pieces(
        &mut out,
        doc.collections
            .iter()
            .map(collection_piece)
            .collect::<Vec<_>>(),
    );
    out.push_str(DOC_MIDDLE);
    push_pieces(
        &mut out,
        doc.records.iter().map(record_piece).collect::<Vec<_>>(),
    );
    out.push_str(&doc_tail(&doc.source));
    out
}

/// The pieces joined by [`DOC_SEPARATOR`], with no leading or trailing separator: an
/// empty list writes nothing at all, so a document with no collections still has one
/// `[]` rather than a stray comma.
fn push_pieces(out: &mut String, pieces: Vec<String>) {
    for (i, piece) in pieces.iter().enumerate() {
        if i > 0 {
            out.push_str(DOC_SEPARATOR);
        }
        out.push_str(piece);
    }
}

/// The canonical wire text of one value: no newline, no enclosing document.
pub fn to_json_value(value: &JsonValue) -> String {
    let mut out = String::new();
    write_value(&mut out, value);
    out
}

/// One value read back, or the JSON fault. The one place a cell can arrive from text.
pub fn read_value(text: &str) -> Result<JsonValue, super::IngestError> {
    super::read::cell(
        &crate::canonical_json::parse(text).map_err(super::IngestError::from_json)?,
        "",
    )
}

pub(crate) fn collection(c: &Collection) -> String {
    let mut out = String::new();
    write_members(
        &mut out,
        &[
            ("fields", fields(&c.fields)),
            ("id", quoted(&c.id)),
            ("name", quoted(&c.name)),
            ("titleField", quoted(&c.title_field)),
        ],
    );
    out
}

fn fields(items: &[Field]) -> String {
    // Sorted by field id, matching the reader's canonical order: a document written by
    // hand in any order and one read back are the same text.
    let mut sorted: Vec<&Field> = items.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    let rows: Vec<String> = sorted.iter().map(|f| field(f)).collect();
    format!("[{}]", rows.join(","))
}

fn field(f: &Field) -> String {
    let mut out = String::new();
    write_members(
        &mut out,
        &[
            ("id", quoted(&f.id)),
            (
                "link",
                f.link.as_ref().map_or_else(|| "null".to_owned(), link_json),
            ),
            ("name", quoted(&f.name)),
            ("role", quoted(f.role.as_str())),
        ],
    );
    out
}

/// A `Link` as its object text. Named here so `field` stays a four-row table.
fn link_json(link: &super::Link) -> String {
    let mut out = String::new();
    write_members(
        &mut out,
        &[
            ("cardinality", quoted(link.cardinality.as_str())),
            ("collection", quoted(&link.collection)),
            ("symmetric", link.symmetric.to_string()),
        ],
    );
    out
}

pub(crate) fn record(r: &super::Record) -> String {
    let mut out = String::new();
    write_members(&mut out, &record_rows(r));
    out
}

/// The record's own members, already in key order, as `(key, value text)`.
///
/// The list rather than the joined text, because a second writer — the hub's change
/// writer — needs to write the same record with one member *added* (`rev`). If it rebuilt
/// the member table it would be a second list of a record's members to keep in step with
/// this one; if it appended, its bytes would not be in key order. So the table is made
/// once, here, and both writers join it.
pub(crate) fn record_rows(r: &super::Record) -> Vec<(&'static str, String)> {
    vec![
        ("collection", quoted(&r.collection)),
        ("deleted", r.deleted.to_string()),
        ("id", quoted(&r.id)),
        ("updatedAt", r.updated_at.to_string()),
        ("values", members(&cells(r))),
    ]
}

/// One `(field id, value text)` row per *distinct* field id, in the order the cells
/// first appear.
///
/// `Record::values` is a `Vec` of pairs with every field `pub`, so a Rust-built record
/// can hold two cells for one id — and writing both would emit `"t":"a","t":"b"`, which
/// `canonical_json::parse` refuses ("a key repeated in one object"). So a repeat
/// collapses to the FIRST, in document order, before the sort: that is what
/// `Record::value` already resolves to (`find`), so the writer never contradicts the
/// lookup a caller does on the same struct.
fn cells(r: &super::Record) -> Vec<(&str, String)> {
    let mut seen: Vec<&str> = Vec::new();
    let mut rows: Vec<(&str, String)> = Vec::new();
    for (id, value) in &r.values {
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id.as_str());
        rows.push((id.as_str(), to_json_value(value)));
    }
    // Sorted by key, then by value for two equal keys — a total order, so the text is
    // the same whatever order the document was read in (D5, H6).
    rows.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
    rows
}

/// `{...}` from rows already in key order. The caller sorts; this only joins, so the
/// two concerns cannot drift apart. **The braces are included**, so a caller writes
/// `members(&rows)` where it wants an object and does not add its own.
pub(crate) fn members(rows: &[(&str, String)]) -> String {
    let mut out = String::new();
    write_members(&mut out, rows);
    out
}

fn write_members(out: &mut String, rows: &[(&str, String)]) {
    out.push('{');
    for (i, (key, value)) in rows.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quoted(key));
        out.push(':');
        out.push_str(value);
    }
    out.push('}');
}

/// A JSON string literal, escaping exactly what RFC 8259 requires and nothing else:
/// the quote, the backslash, and every control character below `U+0020` (as the short
/// escape where one exists, `\u00XX` otherwise). Every other character — including
/// `U+007F`, `/`, and everything non-ASCII — is written literally as UTF-8, which is
/// what a JavaScript `JSON.stringify` reader sees identically.
pub(crate) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_control_character_below_twenty_writes_as_a_four_hex_escape() {
        assert_eq!(quoted("\u{1}"), "\"\\u0001\"");
        assert_eq!(quoted("\u{1f}"), "\"\\u001f\"");
    }

    #[test]
    fn the_named_short_escapes_are_used_where_they_exist() {
        assert_eq!(quoted("\u{8}\u{c}"), "\"\\b\\f\"");
        assert_eq!(quoted("\n\r\t"), "\"\\n\\r\\t\"");
    }

    #[test]
    fn a_delete_is_literal_because_json_does_not_require_escaping_it() {
        assert_eq!(quoted("\u{7f}"), "\"\u{7f}\"");
    }

    #[test]
    fn a_quote_and_a_backslash_are_escaped_and_nothing_else_is() {
        assert_eq!(quoted("a\"b\\c/d"), "\"a\\\"b\\\\c/d\"");
    }
}
