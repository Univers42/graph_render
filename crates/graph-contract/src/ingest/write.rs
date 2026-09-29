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

/// The canonical wire text of one document.
pub fn to_json(doc: &Ingest) -> String {
    let mut out = String::new();
    write_members(
        &mut out,
        &[
            ("collections", collections(&doc.collections)),
            ("records", records(&doc.records)),
            ("source", quoted(&doc.source)),
            ("version", doc.version.to_string()),
        ],
    );
    out.push('\n');
    out
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

fn collections(items: &[Collection]) -> String {
    let rows: Vec<String> = items.iter().map(collection).collect();
    format!("[{}]", rows.join(","))
}

fn collection(c: &Collection) -> String {
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

fn records(items: &[super::Record]) -> String {
    let rows: Vec<String> = items.iter().map(record).collect();
    format!("[{}]", rows.join(","))
}

fn record(r: &super::Record) -> String {
    let mut cells: Vec<(&str, String)> = r
        .values
        .iter()
        .map(|(id, value)| (id.as_str(), to_json_value(value)))
        .collect();
    // Sorted by key, then by value for two equal keys — a total order, so the text is
    // the same whatever order the document was read in (D5, H6).
    cells.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
    let mut out = String::new();
    write_members(
        &mut out,
        &[
            ("collection", quoted(&r.collection)),
            ("deleted", r.deleted.to_string()),
            ("id", quoted(&r.id)),
            ("updatedAt", r.updated_at.to_string()),
            ("values", members(&cells)),
        ],
    );
    out
}

/// `{...}` from rows already in key order. The caller sorts; this only joins, so the
/// two concerns cannot drift apart.
fn members(rows: &[(&str, String)]) -> String {
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

fn write_value(out: &mut String, value: &JsonValue) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(n) => {
            // Rust's `Display` for f64 is the shortest decimal that reads back as the
            // same value, and it writes a whole number without a fractional part — so
            // `2.0` and `2` are one text, which is what keeps two adapters' documents
            // byte-identical rather than merely equal.
            let _ = write!(out, "{n}");
        }
        JsonValue::Text(text) => out.push_str(&quoted(text)),
        JsonValue::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(out, item);
            }
            out.push(']');
        }
        JsonValue::Map(members) => {
            let mut rows: Vec<(&str, String)> = members
                .iter()
                .map(|(k, v)| {
                    (k.as_str(), {
                        let mut text = String::new();
                        write_value(&mut text, v);
                        text
                    })
                })
                .collect();
            rows.sort_by(|a, b| a.0.cmp(b.0).then_with(|| a.1.cmp(&b.1)));
            out.push('{');
            for (i, (key, text)) in rows.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&quoted(key));
                out.push(':');
                out.push_str(text);
            }
            out.push('}');
        }
    }
}

/// A JSON string literal, escaping exactly what RFC 8259 requires and nothing else:
/// the quote, the backslash, and every control character below `U+0020` (as the short
/// escape where one exists, `\u00XX` otherwise). Every other character — including
/// `U+007F`, `/`, and everything non-ASCII — is written literally as UTF-8, which is
/// what a JavaScript `JSON.stringify` reader sees identically.
fn quoted(text: &str) -> String {
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
