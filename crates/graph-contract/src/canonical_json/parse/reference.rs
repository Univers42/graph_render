//! The reader as it stood before the shared scratch stacks: every container grew its own
//! `Vec` by doubling, and every object's duplicate-key check built a `BTreeSet`. Frozen as
//! the oracle for [`super::differential`]. It must not be changed to agree with the new
//! one; if they disagree, one is wrong and the test says which input.

use crate::canonical_json::{JsonError, Value};
use std::collections::BTreeSet;

pub(super) fn parse(text: &str) -> Result<Value, JsonError> {
    let mut p = Parser { text, at: 0 };
    let value = p.value(0)?;
    p.skip_space();
    if p.at != text.len() {
        return Err(p.fault("text after the value"));
    }
    Ok(value)
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn fault(&self, what: &'static str) -> JsonError {
        let at = u32::try_from(self.at).unwrap_or(u32::MAX);
        JsonError::Syntax { at, what }
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn skip_space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> bool {
        let found = self.peek() == Some(byte);
        self.at += usize::from(found);
        found
    }

    fn value(&mut self, depth: u32) -> Result<Value, JsonError> {
        if depth > super::MAX_DEPTH {
            return Err(self.fault("nested too deep"));
        }
        self.skip_space();
        match self.peek() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.number().map(Value::Number),
            Some(b't') => self.literal("true", Value::Bool(true)),
            Some(b'f') => self.literal("false", Value::Bool(false)),
            Some(b'n') => self.literal("null", Value::Null),
            Some(_) => Err(self.fault("not the start of a value")),
            None => Err(self.fault("the text ends where a value should be")),
        }
    }

    fn literal(&mut self, word: &str, value: Value) -> Result<Value, JsonError> {
        if !self.text[self.at..].starts_with(word) {
            return Err(self.fault("not the start of a value"));
        }
        self.at += word.len();
        Ok(value)
    }

    fn array(&mut self, depth: u32) -> Result<Value, JsonError> {
        self.at += 1;
        let mut items = Vec::new();
        self.skip_space();
        if self.eat(b']') {
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth + 1)?);
            self.skip_space();
            if self.eat(b']') {
                return Ok(Value::Array(items));
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or ] in an array"));
            }
        }
    }

    fn object(&mut self, depth: u32) -> Result<Value, JsonError> {
        self.at += 1;
        let mut members: Vec<(String, Value)> = Vec::new();
        let mut keys = BTreeSet::new();
        self.skip_space();
        if self.eat(b'}') {
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_space();
            if self.peek() != Some(b'"') {
                return Err(self.fault("expected a key"));
            }
            let key_at = self.at;
            let key = self.string()?;
            self.skip_space();
            if !self.eat(b':') {
                return Err(self.fault("expected : after a key"));
            }
            let value = self.value(depth + 1)?;
            if !keys.insert(key.clone()) {
                self.at = key_at;
                return Err(self.fault("a key repeated in one object"));
            }
            members.push((key, value));
            self.skip_space();
            if self.eat(b'}') {
                return Ok(Value::Object(members));
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or } in an object"));
            }
        }
    }

    fn number(&mut self) -> Result<String, JsonError> {
        let start = self.at;
        self.eat(b'-');
        if !self.eat(b'0') && self.digits() == 0 {
            return Err(self.fault("a number needs digits"));
        }
        if self.eat(b'.') && self.digits() == 0 {
            return Err(self.fault("a fraction needs digits"));
        }
        if self.eat(b'e') || self.eat(b'E') {
            let _ = self.eat(b'+') || self.eat(b'-');
            if self.digits() == 0 {
                return Err(self.fault("an exponent needs digits"));
            }
        }
        Ok(self.text[start..self.at].to_owned())
    }

    fn digits(&mut self) -> usize {
        let start = self.at;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.at += 1;
        }
        self.at - start
    }

    /// One JSON string, unescaped — a copy of the reader's, not a call into it: a frozen
    /// reference that shares the scanner under test cannot catch a fault in the scanner.
    fn string(&mut self) -> Result<String, JsonError> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let run = self.at;
            while matches!(self.peek(), Some(b) if b != b'"' && b != b'\\' && b >= 0x20) {
                self.at += 1;
            }
            out.push_str(&self.text[run..self.at]);
            match self.peek() {
                Some(b'"') => {
                    self.at += 1;
                    return Ok(out);
                }
                Some(b'\\') => out.push(self.escape()?),
                Some(_) => return Err(self.fault("a raw control character in a string")),
                None => return Err(self.fault("the text ends inside a string")),
            }
        }
    }

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
