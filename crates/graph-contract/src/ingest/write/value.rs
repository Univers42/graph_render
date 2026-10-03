//! The canonical text of one [`JsonValue`], the leaf of the writer.
//!
//! Split out of `write.rs` by the house's 40-line limit on a function: a value is
//! recursive and each arm needs its own note (a number's shortest form, a map's key
//! order), so it is a module of its own rather than one long match in the middle of a
//! document writer. Nothing here knows about `Ingest`.

use super::{JsonValue, quoted};
use core::fmt::Write;

/// The canonical wire text of one value, appended to `out`: no newline, no enclosing
/// document. Recurses, and sorts a map's members the way `write.rs` sorts a record's.
pub(super) fn value(out: &mut String, v: &JsonValue) {
    match v {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(n) => number(out, *n),
        JsonValue::Text(text) => out.push_str(&quoted(text)),
        JsonValue::List(items) => list(out, items),
        JsonValue::Map(members) => map(out, members),
    }
}

/// Rust's `Display` for f64 is the shortest decimal that reads back as the same value,
/// and it writes a whole number without a fractional part — so `2.0` and `2` are one
/// text, which is what keeps two adapters' documents byte-identical rather than merely
/// equal. The `let _` is because a `String` can never fail to accept a number.
fn number(out: &mut String, n: f64) {
    let _ = write!(out, "{n}");
}

/// `[...]` in document order: an array's order is meaning, never sorted.
fn list(out: &mut String, items: &[JsonValue]) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        value(out, item);
    }
    out.push(']');
}

/// `{...}` sorted by key, then by value for two equal keys — the same total order
/// `write.rs` uses for a record's `values`, so the two cannot drift apart (D5, H6).
fn map(out: &mut String, members: &[(String, JsonValue)]) {
    let mut rows: Vec<(&str, String)> = members
        .iter()
        .map(|(key, v)| {
            let mut text = String::new();
            value(&mut text, v);
            (key.as_str(), text)
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
