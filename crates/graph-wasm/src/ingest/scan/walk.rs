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

use crate::ingest::element::Element;

use super::{Field, IngestError, MAX_DEPTH, Member, Scan, Span, Text, WIDE_OBJECT, span};

impl<'a> Scan<'a> {
    /// The whole text as one strict RFC 8259 value, with the root object's members —
    /// unescaped key, value span, array element count — located by the walk that refuses
    /// a fault in them, and whether the root was an object at all.
    ///
    /// The capture rides along with the validation instead of following it. Locating the
    /// root used to be a second walk over the whole text, and it could not learn anything
    /// the first had not already validated: at 1M nodes it was 17 % of `extend`'s
    /// instructions for nothing (`docs/measurements/perf-p4d-extend.md`). What it may not do
    /// is change what comes out first, so this refuses exactly where `value` refused and
    /// checks for trailing bytes at exactly the point it did.
    pub(in crate::ingest) fn root(&mut self) -> Result<(bool, Vec<Member>), JsonError> {
        self.space();
        let object = self.byte(self.at) == Some(b'{');
        let mut members = Vec::new();
        if object {
            self.object(0, &mut |field: Field<'_>, elements| {
                members.push(Member {
                    key: field.key.into_string(),
                    value: field.value,
                    elements,
                });
            })?;
        } else {
            self.value(0)?;
        }
        self.space();
        if self.at != self.text.len() {
            return Err(self.fault("text after the value"));
        }
        Ok((object, members))
    }

    /// Every element of the array at `array`, in order: its own text, and the members
    /// the *same* walk reads out of it in document order, which `keep` is handed beside
    /// it.
    ///
    /// The span came from a root member the validating walk already checked, so the only
    /// refusals that can come out are `keep`'s and the one [`Element::fill`] raises about
    /// the shape. Both outrank anything this walk finds, because a refusal is what the
    /// caller is waiting for and a syntax fault here would be a fault the validating walk
    /// missed — and each is the *last* one raised, which is what the pair of passes this
    /// replaces did: a refusal never stopped either walk, so the last element to fail was
    /// the one named (`docs/measurements/perf-p4d-extend.md`).
    ///
    /// The element's members are walked here and not by [`Self::array`], so each element
    /// is read once. `array` would read it as a value first and hand back a span, and the
    /// record's own pass would read the same bytes again — which is the half of the old
    /// cost that was here to begin with. The array's own shape (brackets, commas, closing)
    /// is therefore spelled out below rather than shared; it is these ten lines.
    pub(in crate::ingest) fn records(
        &mut self,
        array: Span,
        element: &mut Element<'a>,
        keep: &mut impl FnMut(&mut Element<'a>) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        let start = array
            .bounds()
            .map(|(start, _)| start)
            .ok_or_else(|| IngestError::Json(self.fault("a span past the text")))?;
        self.at = start;
        let mut refused: Option<IngestError> = None;
        // The element's position in the list, advanced only once an element has been read
        // into a record. A refused element leaves it where it was, so the element after it
        // is refused at the *same* position — which is what the reader that numbered the
        // elements in its own loop did, and what the frozen reader names.
        let mut index = 0usize;
        self.space();
        if self.eat(b'[') && !self.eat(b']') {
            'elements: loop {
                self.space();
                let from = self.at;
                element.seek(index);
                element.reset();
                match element.fill(self) {
                    Ok(Err(why)) => refused = Some(why),
                    // A fault in the element's own bytes stops the walk, as reading the
                    // element as one value did. Its offset is moved onto the element,
                    // because that is where the pass this replaces counted from.
                    Err(fault) => {
                        refused = Some(IngestError::Json(rebase(fault, from)));
                        break 'elements;
                    }
                    Ok(Ok(())) => {
                        if let Err(why) = keep(element) {
                            refused = Some(why);
                        } else {
                            index += 1;
                        }
                    }
                }
                self.space();
                if self.eat(b']') {
                    break;
                }
                if !self.eat(b',') {
                    return Err(IngestError::Json(self.fault("expected , or ] in an array")));
                }
            }
        }
        match refused {
            Some(why) => Err(why),
            None => Ok(()),
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
            Some(b'{') => {
                self.object(depth, &mut |_, _| ())?;
                None
            }
            Some(b'[') => {
                self.array(depth, &mut |_| ())?;
                None
            }
            Some(b'"') => {
                let (text, end) = self.string(self.at)?;
                self.at = end;
                Some(text)
            }
            Some(b'-' | b'0'..=b'9') => {
                let (_, end) = self.number(self.at)?;
                self.at = end;
                None
            }
            Some(b't') => {
                self.literal("true")?;
                None
            }
            Some(b'f') => {
                self.literal("false")?;
                None
            }
            Some(b'n') => {
                self.literal("null")?;
                None
            }
            Some(_) => return Err(self.fault("not the start of a value")),
            None => return Err(self.fault("the text ends where a value should be")),
        };
        Ok((start, text))
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

/// A syntax fault in one record element, moved onto that element: `at` becomes `at` less
/// the element's first byte.
///
/// The pass this replaces read a record element out of its own text, so every fault it
/// reported inside an element counted from the element and not from the document. The
/// merged walk counts from the document, and the offset is published — a caller shows it
/// to a person — so it is put back rather than changed.
///
/// An `at` already inside the element cannot happen: the validating walk read these bytes
/// once and refused nothing. `saturating_sub` says so rather than trusting it.
fn rebase(fault: JsonError, from: usize) -> JsonError {
    match fault {
        JsonError::Syntax { at, what } => JsonError::Syntax {
            at: u32::try_from(u64::from(at).saturating_sub(from as u64)).unwrap_or(at),
            what,
        },
        other => other,
    }
}
