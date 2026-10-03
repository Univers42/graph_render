//! Strings, escapes and numbers for [`super::Scan`]: the lexical half of the walk, split
//! out so the container machinery (the root's members, the element index) reads on its
//! own.
//!
//! Deliberately a mirror of `graph_contract::canonical_json::parse` and its `strings`
//! module — same acceptance, same `JsonError::Syntax` offsets, same messages. The ingest
//! reader validates with this instead of `parse` so no `Value` tree is ever built, and the
//! two must therefore refuse the same bytes for the same reason at the same offset; the
//! differential test in [`super::differential`] is what holds them to it.
//!
//! A string borrows the document unless it carries an escape, in which case it is built and
//! owned ([`Text::Owned`]) — the common case costs no allocation at all, which is the whole
//! point of the walk.

use super::{JsonError, Scan, Text};

impl<'a> Scan<'a> {
    /// One JSON string at `start` (its opening quote): the unescaped text and one past its
    /// closing quote. A run with no escape borrows the document.
    pub(in crate::ingest) fn string(
        &mut self,
        start: usize,
    ) -> Result<(Text<'a>, usize), JsonError> {
        let mut at = start + 1;
        let run = at;
        while matches!(self.byte(at), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
            at += 1;
        }
        match self.byte(at) {
            Some(b'"') => Ok((Text::borrowed(&self.text[run..at]), at + 1)),
            Some(b'\\') => self.escaped(run, at),
            Some(_) => Err(self.fault_at(at, "a raw control character in a string")),
            None => Err(self.fault_at(at, "the text ends inside a string")),
        }
    }

    /// A string carrying at least one escape: the run before it, then one escape and one
    /// run at a time. `at` is the backslash that broke the first run.
    pub(in crate::ingest) fn escaped(
        &self,
        run: usize,
        at: usize,
    ) -> Result<(Text<'a>, usize), JsonError> {
        let mut out = self.text[run..at].to_owned();
        let mut at = at;
        loop {
            let letter = self
                .byte(at + 1)
                .ok_or_else(|| self.fault_at(at + 1, "the text ends inside a string"))?;
            at += 2;
            match letter {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let (c, next) = self.unicode(at)?;
                    out.push(c);
                    at = next;
                }
                // One past the letter, which is where `parse` stands when it refuses: its
                // `escape` has already stepped over both bytes of the sequence.
                _ => return Err(self.fault_at(at, "an unknown escape")),
            }
            // The run of plain bytes up to the next break, appended whole: the escape just
            // read stands where the run begins, so nothing between them is skipped.
            let run = at;
            while matches!(self.byte(at), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
                at += 1;
            }
            out.push_str(&self.text[run..at]);
            match self.byte(at) {
                Some(b'"') => return Ok((Text::owned(out), at + 1)),
                Some(b'\\') => {}
                Some(_) => return Err(self.fault_at(at, "a raw control character in a string")),
                None => return Err(self.fault_at(at, "the text ends inside a string")),
            }
        }
    }

    /// `\uXXXX` at `at`, pairing a high surrogate with the low one that must follow it:
    /// the character, and one past the whole sequence.
    pub(in crate::ingest) fn unicode(&self, at: usize) -> Result<(char, usize), JsonError> {
        let (unit, mut at) = self.hex4(at)?;
        let code = match unit {
            0xD800..=0xDBFF => {
                if !self.text[at..].starts_with("\\u") {
                    return Err(self.fault_at(at, "an unpaired surrogate"));
                }
                let (low, next) = self.hex4(at + 2)?;
                at = next;
                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(self.fault_at(at, "an unpaired surrogate"));
                }
                0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00)
            }
            0xDC00..=0xDFFF => return Err(self.fault_at(at, "an unpaired surrogate")),
            _ => unit,
        };
        let end = at;
        char::from_u32(code)
            .map(|c| (c, end))
            .ok_or_else(|| self.fault_at(at, "an unpaired surrogate"))
    }

    /// Four hex digits at `at`: their value, and one past them.
    pub(in crate::ingest) fn hex4(&self, at: usize) -> Result<(u32, usize), JsonError> {
        let digits = self.text.get(at..at + 4);
        let value = digits
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .ok_or_else(|| self.fault_at(at, "\\u needs four hex digits"))?;
        Ok((value, at + 4))
    }

    /// One JSON number at `start`: its text as written, and one past it. The text is kept
    /// rather than the value so `version` can refuse `1.0` and `1e0`, which a reader that
    /// rounded would read as `1`.
    ///
    /// Every refusal is reported *past* the sign, the point or the exponent marker rather
    /// than at the number's first byte, because that is where the reader this mirrors stands
    /// when it refuses — it has already stepped over what it read.
    pub(in crate::ingest) fn number(&self, start: usize) -> Result<(&str, usize), JsonError> {
        let mut at = start;
        if self.byte(at) == Some(b'-') {
            at += 1;
        }
        if self.byte(at) == Some(b'0') {
            at += 1;
        } else if self.digits(at) == 0 {
            return Err(self.fault_at(at, "a number needs digits"));
        } else {
            at += self.digits(at);
        }
        if self.byte(at) == Some(b'.') {
            at += 1;
            if self.digits(at) == 0 {
                return Err(self.fault_at(at, "a fraction needs digits"));
            }
            at += self.digits(at);
        }
        if matches!(self.byte(at), Some(b'e' | b'E')) {
            at += 1;
            if matches!(self.byte(at), Some(b'+' | b'-')) {
                at += 1;
            }
            if self.digits(at) == 0 {
                return Err(self.fault_at(at, "an exponent needs digits"));
            }
            at += self.digits(at);
        }
        Ok((&self.text[start..at], at))
    }

    /// How many decimal digits start at `at`.
    pub(in crate::ingest) fn digits(&self, at: usize) -> usize {
        let mut end = at;
        while matches!(self.byte(end), Some(b'0'..=b'9')) {
            end += 1;
        }
        end - at
    }
}
