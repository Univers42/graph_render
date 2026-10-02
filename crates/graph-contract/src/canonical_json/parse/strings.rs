//! Strings and their escapes: the scanner's only place a `\u` sequence, a surrogate pair
//! or a raw control byte is decided. Split out of `parse.rs` so the container machinery
//! (the two scratch stacks) and the lexical machinery read separately.

use crate::canonical_json::JsonError;

/// One JSON string, unescaped, read from `p`. A run with no escape is one `push_str`; an
/// escape breaks the run and appends the character it names. Shared with the frozen
/// reference reader, so a string is scanned by the same code in both.
pub(super) fn read_string(p: &mut super::Parser<'_>) -> Result<String, JsonError> {
    p.at += 1;
    let mut out = String::new();
    loop {
        let run = p.at;
        while matches!(p.peek(), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
            p.at += 1;
        }
        out.push_str(&p.text[run..p.at]);
        match p.peek() {
            Some(b'"') => {
                p.at += 1;
                return Ok(out);
            }
            Some(b'\\') => out.push(p.escape()?),
            Some(_) => return Err(p.fault("a raw control character in a string")),
            None => return Err(p.fault("the text ends inside a string")),
        }
    }
}

impl super::Parser<'_> {
    fn escape(&mut self) -> Result<char, JsonError> {
        self.at += 1;
        let letter = self
            .peek()
            .ok_or_else(|| self.fault("the text ends inside a string"))?;
        self.at += 1;
        Ok(match letter {
            b'"' => '"',
            b'\\' => '\\',
            b'/' => '/',
            b'b' => '\u{8}',
            b'f' => '\u{c}',
            b'n' => '\n',
            b'r' => '\r',
            b't' => '\t',
            b'u' => return self.unicode(),
            _ => return Err(self.fault("an unknown escape")),
        })
    }

    /// `\uXXXX`, pairing a high surrogate with the low one that must follow it.
    fn unicode(&mut self) -> Result<char, JsonError> {
        let unit = self.hex4()?;
        let code = match unit {
            0xD800..=0xDBFF => {
                if !self.text[self.at..].starts_with("\\u") {
                    return Err(self.fault("an unpaired surrogate"));
                }
                self.at += 2;
                match self.hex4()? {
                    low @ 0xDC00..=0xDFFF => 0x10000 + ((unit - 0xD800) << 10) + (low - 0xDC00),
                    _ => return Err(self.fault("an unpaired surrogate")),
                }
            }
            0xDC00..=0xDFFF => return Err(self.fault("an unpaired surrogate")),
            _ => unit,
        };
        char::from_u32(code).ok_or_else(|| self.fault("an unpaired surrogate"))
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let digits = self.text.get(self.at..self.at + 4);
        let value = digits
            .filter(|d| d.bytes().all(|b| b.is_ascii_hexdigit()))
            .and_then(|d| u32::from_str_radix(d, 16).ok())
            .ok_or_else(|| self.fault("\\u needs four hex digits"))?;
        self.at += 4;
        Ok(value)
    }
}
