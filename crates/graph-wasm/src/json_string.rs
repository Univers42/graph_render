//! A JSON string literal, written once for every hand-built JSON face in this crate
//! (`analysis`'s report, `seed_ingest`'s document). graph-contract's canonical writer has
//! the same escaper but keeps it private.

use std::fmt::Write as _;

/// Appends `text` to `out` as a quoted JSON string: `"` and `\` escaped, every control
/// character below `0x20` written as `\u00XX`, everything else verbatim (RFC 8259 §7).
pub(crate) fn push_quoted(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
