//! Batch bodies of a known size, one per shape the reader can be fed, for the memory cases.
//!
//! Every body is `{"upserts":[…],"deletes":[]}` whose records carry one `note` (role `scalar`,
//! which accepts any JSON) holding a list or a map of tiny items. Tiny items are the reader's worst
//! case: its parse tree costs a heap node per item however short the item's text. `records` spends
//! the body on record overhead instead, up to the 10 000-operation cap.

use std::fmt::Write;

/// Every shape, in the order the report lists them.
pub const SHAPES: [&str; 5] = ["zeros", "strings", "nested", "keys", "records"];

/// Payload bytes per record in the one-payload shapes: half the 1 MiB record cap, so a record's
/// canonical text stays under it.
const PER_RECORD: usize = 512 << 10;

/// Body bytes per record in `records`: about half head, half payload.
const RECORD_BYTES: usize = 256;

const HEAD: &str = r#"{"upserts":["#;
const TAIL: &str = r#"],"deletes":[]}"#;

/// A body in `shape` of at most `size` bytes, filled to within one item of it.
pub fn body(shape: &str, size: usize, max_batch: usize) -> String {
    let records = if shape == "records" {
        (size / RECORD_BYTES).clamp(1, max_batch)
    } else {
        size.div_ceil(PER_RECORD).max(1)
    };
    let budget = (size - HEAD.len() - TAIL.len()) / records;
    let mut out = String::with_capacity(size);
    out.push_str(HEAD);
    for i in 0..records {
        record(&mut out, shape, i, budget);
    }
    out.push_str(TAIL);
    assert!(
        out.len() <= size,
        "a {shape} body of {} > {size}",
        out.len()
    );
    out
}

/// Record `i`, at most `budget` bytes with its separator, items added while they fit.
fn record(out: &mut String, shape: &str, i: usize, budget: usize) {
    let start = out.len();
    if i > 0 {
        out.push(',');
    }
    let (open, close) = if shape == "keys" {
        ('{', '}')
    } else {
        ('[', ']')
    };
    let _ = write!(
        out,
        r#"{{"collection":"task","id":"r{i}","updatedAt":0,"values":{{"note":{open}"#
    );
    let mut item = String::new();
    for n in 0.. {
        item.clear();
        if n > 0 {
            item.push(',');
        }
        push_item(&mut item, shape, n);
        if out.len() - start + item.len() + "]}}".len() > budget {
            break;
        }
        out.push_str(&item);
    }
    out.push(close);
    out.push_str("}}");
}

/// Item `n` of `shape`'s payload.
fn push_item(item: &mut String, shape: &str, n: usize) {
    match shape {
        "zeros" | "records" => item.push('0'),
        "strings" => item.push_str(r#""""#),
        "nested" => item.push_str("[]"),
        "keys" => {
            let _ = write!(item, r#""k{n}":0"#);
        }
        other => panic!("no body shape {other}"),
    }
}
