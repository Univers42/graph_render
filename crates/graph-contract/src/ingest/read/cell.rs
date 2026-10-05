//! One record cell, as a [`JsonValue`]: the whole of JSON and nothing more, so a source's
//! own types survive the trip.
//!
//! Split out of `read.rs` for the house's 300-line limit, and because the number branch
//! carries the contract's own arithmetic rules — D9's non-finite refusal and the
//! integer-exactness refusal below — which are worth a file of their own rather than a
//! clause in a member-by-member walk.

use super::shape;
use crate::canonical_json::Value;
use crate::ingest::{IngestError, JsonValue};

/// One cell. The number branch is where D9 is enforced: the JSON grammar keeps out
/// `NaN`/`Infinity`, but an exponent that overflows `f64` parses as text, so it is
/// refused here rather than on a derived field — and an integer the `f64` cannot hold
/// exactly is refused rather than rounded (see [`mutates_the_text`]).
pub(crate) fn cell(value: &Value, path: &str) -> Result<JsonValue, IngestError> {
    Ok(match value {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => JsonValue::Bool(*b),
        Value::Number(text) => number(text, path)?,
        Value::String(text) => JsonValue::Text(text.clone()),
        Value::Array(items) => JsonValue::List(
            items
                .iter()
                .enumerate()
                .map(|(i, v)| cell(v, &format!("{path}[{i}]")))
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(members) => JsonValue::Map(
            members
                .iter()
                .map(|(k, v)| cell(v, &format!("{path}.{k}")).map(|value| (k.clone(), value)))
                .collect::<Result<_, _>>()?,
        ),
    })
}

fn number(text: &str, path: &str) -> Result<JsonValue, IngestError> {
    let n: f64 = text
        .parse()
        .map_err(|_| shape(path, "not a valid number"))?;
    if !n.is_finite() {
        return Err(shape(path, "not finite"));
    }
    if mutates_the_text(text, n) {
        return Err(shape(path, EXACT_INT_FAULT.to_owned()));
    }
    Ok(JsonValue::Number(n))
}

const EXACT_INT_FAULT: &str = "an integer past 9007199254740992 cannot be read exactly";

/// Whether reading the bare integer `text` as the `f64` `n` would rewrite the document's
/// own bytes. [`JsonValue::Number`] is an `f64` by contract, so an integer whose value the
/// `f64` cannot hold lands on a neighbour: `9007199254740993` reads as `9007199254740992`,
/// and the cell is silently a different number with nothing said.
///
/// The test is the **canonical spelling**, not a magnitude: `n` writes back as the
/// shortest decimal that reads as `n`, and that is the writer's own text whenever the two
/// agree. So `1e21`, which the writer spells as the bare integer
/// `1000000000000000000000` and which no `f64` holds exactly, still reads — that text is
/// the only spelling of the number the writer meant, and refusing it would break
/// write-then-read. A document that is *not* that text for the number it reads as
/// (`1000000000000000000001`) is refused. A float literal is left alone: an exponent says
/// its own precision, and rounding one gives the float the document asked for.
fn mutates_the_text(text: &str, n: f64) -> bool {
    if text.contains('.') || text.contains('e') || text.contains('E') {
        return false;
    }
    n.to_string() != text
}
