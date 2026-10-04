//! The containers and the cursor: values, arrays, objects, and the byte-level moves every
//! one of them is made of. Split from the parent module for the house line limit; `Scan`'s
//! fields are its own, the lexical half is [`super::text`], and the two whole-document
//! passes that drive these are [`pass`].
//!
//! A deliberate line-for-line mirror of `graph_contract::canonical_json::parse`, down to the
//! order a member's value is read before its key is checked for repetition, the offset a
//! repeated key is reported at, and the depth each container's members are read at. That is
//! what makes "the same refusals, at the same byte offsets" a property of the code rather
//! than of the differential test.

mod pass;

use graph_contract::canonical_json::JsonError;

use super::{Field, MAX_DEPTH, Scan, Span, Text, WIDE_OBJECT, span};

impl<'a> Scan<'a> {
    /// `JsonError::Syntax` at the cursor, as `parse` reports it.
    pub(in crate::ingest) fn fault(&self, what: &'static str) -> JsonError {
        self.fault_at(self.at, what)
    }

    /// `JsonError::Syntax` at `at`.
    pub(in crate::ingest) fn fault_at(&self, at: usize, what: &'static str) -> JsonError {
        JsonError::Syntax {
            at: u32::try_from(at).unwrap_or(u32::MAX),
            what,
        }
    }

    /// The byte at `at`, or `None` past the end.
    pub(in crate::ingest) fn byte(&self, at: usize) -> Option<u8> {
        self.text.as_bytes().get(at).copied()
    }

    /// Past the whitespace, where a value or a member name may start.
    pub(in crate::ingest) fn space(&mut self) {
        while matches!(self.byte(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// Eats `byte` if it is next, moving the cursor only when it was.
    pub(in crate::ingest) fn eat(&mut self, byte: u8) -> bool {
        let found = self.byte(self.at) == Some(byte);
        self.at += usize::from(found);
        found
    }

    /// One JSON value at the cursor, at `depth`: where it starts, and — for a value that
    /// is a string — that string's own text, so a reader of it need not lex these bytes
    /// again. The cursor lands just past the value either way.
    pub(in crate::ingest) fn value_text(
        &mut self,
        depth: u32,
    ) -> Result<(usize, Option<Text<'a>>), JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.fault("nested too deep"));
        }
        self.space();
        let start = self.at;
        let text = match self.byte(self.at) {
            Some(b'"') => Some(self.string_value()?),
            Some(b'{') => {
                self.object(depth, &mut |_, _| ())?;
                None
            }
            Some(b'[') => {
                self.array(depth, &mut |_| ())?;
                None
            }
            _ => {
                self.plain_value()?;
                None
            }
        };
        Ok((start, text))
    }

    /// The string at the cursor, and the cursor just past it.
    fn string_value(&mut self) -> Result<Text<'a>, JsonError> {
        let (text, end) = self.string(self.at)?;
        self.at = end;
        Ok(text)
    }

    /// A number, or one of `null`/`true`/`false`, at the cursor; the cursor just past it.
    /// Never a string, which is why the caller wraps this in a `None` of its own.
    fn plain_value(&mut self) -> Result<(), JsonError> {
        match self.byte(self.at) {
            Some(b'-' | b'0'..=b'9') => {
                let (_, end) = self.number(self.at)?;
                self.at = end;
            }
            Some(b't') => self.literal("true")?,
            Some(b'f') => self.literal("false")?,
            Some(b'n') => self.literal("null")?,
            Some(_) => return Err(self.fault("not the start of a value")),
            None => return Err(self.fault("the text ends where a value should be")),
        }
        Ok(())
    }

    /// [`Self::value_text`], keeping only where the value was.
    pub(in crate::ingest) fn value(&mut self, depth: u32) -> Result<Span, JsonError> {
        let start = self.value_text(depth)?.0;
        Ok(span(start, self.at))
    }

    /// One of `null`/`true`/`false` at the cursor.
    pub(in crate::ingest) fn literal(&mut self, word: &str) -> Result<(), JsonError> {
        if !self.text[self.at..].starts_with(word) {
            return Err(self.fault("not the start of a value"));
        }
        self.at += word.len();
        Ok(())
    }

    /// One array at `depth`, handing each element's span to `keep`.
    pub(in crate::ingest) fn array(
        &mut self,
        depth: u32,
        mut keep: impl FnMut(Span),
    ) -> Result<(), JsonError> {
        self.at += 1;
        self.space();
        if self.eat(b']') {
            return Ok(());
        }
        loop {
            let span = self.value(depth + 1)?;
            keep(span);
            self.space();
            if self.eat(b']') {
                return Ok(());
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or ] in an array"));
            }
            self.space();
        }
    }

    /// One object at `depth`, handing each member to `keep` as a [`Field`] — its
    /// unescaped key, its value's span, and that value's own text if it is a string —
    /// plus the element count if the value is an array.
    pub(in crate::ingest) fn object(
        &mut self,
        depth: u32,
        mut keep: impl FnMut(Field<'a>, Option<usize>),
    ) -> Result<(), JsonError> {
        self.at += 1;
        let base = self.keys.len();
        let mut wide: Option<std::collections::BTreeSet<String>> = None;
        self.space();
        if self.eat(b'}') {
            return Ok(());
        }
        loop {
            self.space();
            if self.byte(self.at) != Some(b'"') {
                return Err(self.fault("expected a key"));
            }
            let key_at = self.at;
            let (key, at) = self.string(self.at)?;
            self.at = at;
            self.space();
            if !self.eat(b':') {
                return Err(self.fault("expected : after a key"));
            }
            self.space();
            let start = self.at;
            let mut elements = None;
            let mut text = None;
            if self.byte(self.at) == Some(b'[') {
                let mut count = 0usize;
                self.array(depth + 1, &mut |_| count += 1)?;
                elements = Some(count);
            } else {
                text = self.value_text(depth + 1)?.1;
            }
            if self.key_seen(base, &mut wide, &key) {
                self.at = key_at;
                return Err(self.fault("a key repeated in one object"));
            }
            self.keys.push(key.clone());
            keep(
                Field {
                    key,
                    value: span(start, self.at),
                    text,
                },
                elements,
            );
            self.space();
            if self.eat(b'}') {
                self.keys.truncate(base);
                return Ok(());
            }
            if !self.eat(b',') {
                return Err(self.fault("expected , or } in an object"));
            }
        }
    }

    /// Whether `key` is already one of this object's members. Narrow objects compare
    /// against the keys on the stack — no allocation, and the ten members of a node cost
    /// forty-five `memcmp`s. A wide one builds the set it always had.
    pub(in crate::ingest) fn key_seen(
        &self,
        base: usize,
        wide: &mut Option<std::collections::BTreeSet<String>>,
        key: &Text<'a>,
    ) -> bool {
        let seen = &self.keys[base..];
        if seen.len() < WIDE_OBJECT {
            return seen.iter().any(|k| k.as_str() == key.as_str());
        }
        let keys = wide.get_or_insert_with(|| seen.iter().map(|k| k.as_str().to_owned()).collect());
        !keys.insert(key.as_str().to_owned())
    }
}
