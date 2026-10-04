//! The containers: values, arrays, objects and the two ways a caller re-walks a span it
//! already has. Split from the parent module for the house line limit; `Scan`'s fields are
//! its own and the lexical half is [`super::text`].
//!
//! A deliberate line-for-line mirror of `graph_contract::canonical_json::parse`, down to the
//! order a member's value is read before its key is checked for repetition, the offset a
//! repeated key is reported at, and the depth each container's members are read at. That is
//! what makes "the same refusals, at the same byte offsets" a property of the code rather
//! than of the differential test.

use graph_contract::canonical_json::JsonError;

use super::{IngestError, MAX_DEPTH, ROOT_MEMBER_DEPTH, Scan, Span, Text, WIDE_OBJECT, span};

impl<'a> Scan<'a> {
    /// Every element of the array at `array`, in order, handed to `keep` as its own text.
    ///
    /// The span came from a root member this walk already validated, so the only refusal
    /// that can come out is `keep`'s — and it wins over anything the re-walk finds, because
    /// a refusal is what the caller is waiting for and a syntax fault here would be a fault
    /// the first walk missed.
    pub(in crate::ingest) fn elements(
        &mut self,
        array: Span,
        keep: &mut impl FnMut(&'a str) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        let start = array
            .bounds()
            .map(|(start, _)| start)
            .ok_or_else(|| IngestError::Json(self.fault("a span past the text")))?;
        let text = self.text;
        self.at = start;
        let mut refused: Option<IngestError> = None;
        let walked = self
            .array(ROOT_MEMBER_DEPTH, &mut |span: Span| {
                let Some((from, to)) = span.bounds() else {
                    return;
                };
                let Some(element) = text.get(from..to) else {
                    return;
                };
                if let Err(why) = keep(element) {
                    refused = Some(why);
                }
            })
            .map_err(IngestError::Json);
        match refused {
            Some(why) => Err(why),
            None => walked,
        }
    }

    /// Every member of the one object written in this walk's whole text, in document order:
    /// its unescaped key and where its value is. What a record reader wants — the shape's
    /// members as spans, with nothing built per member.
    ///
    /// `keep`'s refusal wins over anything the re-walk finds, for the same reason as in
    /// [`Self::elements`]: the text was validated by [`Document::new`](super::Document::new),
    /// so a syntax fault here would be a fault that walk missed, and the caller is waiting
    /// on the refusal.
    pub(in crate::ingest) fn members(
        &mut self,
        keep: &mut impl FnMut(Text<'a>, Span) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        self.at = 0;
        let mut refused: Option<IngestError> = None;
        let walked = self.object(0, &mut |key, value, _| {
            if let Err(why) = keep(key, value) {
                refused = Some(why);
            }
        });
        match refused {
            Some(why) => Err(why),
            None => walked.map_err(IngestError::Json),
        }
    }

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

    /// One JSON value at the cursor, at `depth`: where it is, and the cursor just past it.
    pub(in crate::ingest) fn value(&mut self, depth: u32) -> Result<Span, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.fault("nested too deep"));
        }
        self.space();
        let start = self.at;
        match self.byte(self.at) {
            Some(b'{') => self.object(depth, &mut |_, _, _| ())?,
            Some(b'[') => self.array(depth, &mut |_| ())?,
            Some(b'"') => {
                let (_, end) = self.string(self.at)?;
                self.at = end;
            }
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

    /// One object at `depth`, handing each member to `keep`: its unescaped key, its value's
    /// span, and the element count if the value is an array.
    pub(in crate::ingest) fn object(
        &mut self,
        depth: u32,
        mut keep: impl FnMut(Text<'a>, Span, Option<usize>),
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
            if self.byte(self.at) == Some(b'[') {
                let mut count = 0usize;
                self.array(depth + 1, &mut |_| count += 1)?;
                elements = Some(count);
            } else {
                self.value(depth + 1)?;
            }
            if self.key_seen(base, &mut wide, &key) {
                self.at = key_at;
                return Err(self.fault("a key repeated in one object"));
            }
            self.keys.push(key.clone());
            keep(key, span(start, self.at), elements);
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
