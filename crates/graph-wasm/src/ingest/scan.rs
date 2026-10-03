//! The ingest document, validated without ever building a `Value` tree.
//!
//! `graph_contract::canonical_json::parse` hands back the whole document as nested `Vec`s
//! of `Value` — one heap block per object member, one per string, one per number. On the
//! studio's own 1M-node documents that tree measured 3.2x the text
//! (`docs/measurements/fix-ingest-scale.md`), and it is the whole reason a document under
//! 1 GiB could not be built inside wasm32's 4 GiB. So the reader walks the text itself and
//! keeps only what the shape pass needs: the root's members, each array's element *count*,
//! and nothing at all per element.
//!
//! Two walks, and the order between them is the point:
//!
//! 1. [`Document::new`] walks the whole text and refuses exactly what `parse` refuses, at
//!    the same byte offsets with the same messages. Past the cursor it holds the root's
//!    member list and nothing else.
//! 2. [`Scan::elements`] walks one array again, handing each element's text straight to the
//!    record reader. No index, no offsets kept, nothing to pre-size.
//!
//! Split in two because the reader this replaces parses the *entire* text before it looks at
//! the root, so a syntax fault in `edges[4000000]` is reported ahead of a missing `version`.
//! One walk that located as it validated would report them the other way round, which the
//! differential test in [`super::differential`] would rightly call a change of refusal
//! order.
//!
//! The container methods are a deliberate line-for-line mirror of `canonical_json::parse`,
//! down to the order a member's value is read before its key is checked for repetition and
//! the offset a repeated key is reported at. That is what makes "the same refusals" a
//! property of the code rather than of the test.
//!
//! **Ponytail:** one scratch `Vec` of keys, sized by the widest object on the current path
//! (10 for a record, 3 for the root) and reused for every element, so a 5M-edge document
//! allocates nothing per member. Failing input: an object wider than [`WIDE_OBJECT`] pays
//! the clone-per-key set the reader this replaces also pays, and would go quadratic if the
//! threshold were raised. Direction: validates and counts, never rewrites and never rounds.
//! Escape hatch: [`Text::Owned`] is the only allocation on the string path, and only for a
//! string carrying an escape.

mod text;

use graph_contract::canonical_json::JsonError;

use super::IngestError;
/// Deepest nesting read, as `graph_contract::canonical_json::parse` reads it.
const MAX_DEPTH: u32 = 32;

/// Member count above which an object's duplicate-key check builds a set of the keys it has
/// read instead of scanning them — the same threshold, and for the same reason, as the
/// reader this replaces: a quadratic over a hostile object is a worse trade than a clone per
/// member.
const WIDE_OBJECT: usize = 32;

/// The depth of a root member's value: the root object is depth 0, so what it holds is
/// depth 1, and [`Scan::array`] is entered there. Fixed rather than stored because the only
/// arrays this walks are root members' — an element's own arrays are read as part of its
/// text, never re-walked.
const ROOT_MEMBER_DEPTH: u32 = 1;

/// One string's unescaped text: borrowed from the document when it carries no escape, built
/// when it does. The escape path is the rare one, so the common one allocates nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Text<'a> {
    /// No escape: a slice of the document.
    Borrowed(&'a str),
    /// At least one escape: the unescaped characters.
    Owned(Box<str>),
}

impl<'a> Text<'a> {
    fn borrowed(text: &'a str) -> Self {
        Self::Borrowed(text)
    }

    fn owned(text: String) -> Self {
        Self::Owned(text.into_boxed_str())
    }

    /// The unescaped text, wherever it lives.
    pub(super) fn as_str(&self) -> &str {
        match self {
            Self::Borrowed(text) => text,
            Self::Owned(text) => text,
        }
    }

    /// The owned form, for a field a `NodeRecord`/`EdgeRecord` keeps.
    pub(super) fn into_string(self) -> String {
        match self {
            Self::Borrowed(text) => text.to_owned(),
            Self::Owned(text) => text.into_string(),
        }
    }
}

/// A byte range in the document, `start`..`end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Span {
    pub(super) start: u32,
    pub(super) end: u32,
}

impl Span {
    /// The range as a `usize` pair, or `None` if either end does not fit — which a document
    /// under the byte ceiling cannot do, and which is refused rather than wrapped (D6).
    pub(super) fn bounds(self) -> Option<(usize, usize)> {
        Some((
            usize::try_from(self.start).ok()?,
            usize::try_from(self.end).ok()?,
        ))
    }
}

/// One root member: its unescaped key, where its value is, and how many elements it has if
/// it is an array. `None` for anything else, so a `nodes` that is not an array is refused
/// by name instead of read as an empty list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Member {
    pub(super) key: String,
    pub(super) value: Span,
    pub(super) elements: Option<usize>,
}

/// A validated ingest document: the root's members, and nothing per element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Document<'a> {
    text: &'a str,
    members: Vec<Member>,
}

impl<'a> Document<'a> {
    /// `text` as strict RFC 8259 — refused exactly as
    /// [`graph_contract::canonical_json::parse`] refuses it — with the root's members
    /// located. A root that is not an object yields no members; [`read_records`](super)
    /// refuses that with a shape error of its own, after this has had its say on the syntax.
    pub(super) fn new(text: &'a str) -> Result<Self, JsonError> {
        let mut scan = Scan::new(text);
        scan.value(0)?;
        scan.space();
        if scan.at != text.len() {
            return Err(scan.fault("text after the value"));
        }
        scan.at = 0;
        let members = scan.root_members()?;
        Ok(Self { text, members })
    }

    /// The document's text, so a reader that already holds it is not handed it twice.
    pub(super) fn text(&self) -> &'a str {
        self.text
    }

    /// The root's members, in document order.
    pub(super) fn members(&self) -> &[Member] {
        &self.members
    }

    /// The member named `key`. A key repeated in one object is a syntax fault already
    /// refused, so there is never more than one.
    pub(super) fn member(&self, key: &str) -> Option<&Member> {
        self.members.iter().find(|m| m.key == key)
    }
}

/// The walk: a byte cursor over the document with the grammar's rules on it. Every method
/// either advances past a whole value or refuses, so a caller can hand it a document it
/// never intends to look at.
pub(super) struct Scan<'a> {
    pub(super) text: &'a str,
    pub(super) at: usize,
    /// Every key this object has read, and every enclosing object's below `base`. One
    /// `Vec` for the whole walk, truncated as objects close: a document of five million
    /// ten-member objects grows it to ten entries once.
    keys: Vec<Text<'a>>,
}

impl<'a> Scan<'a> {
    /// A cursor at the start of `text`.
    pub(super) fn new(text: &'a str) -> Self {
        Self {
            text,
            at: 0,
            keys: Vec::new(),
        }
    }

    /// The root object's members in document order, each with its array element count. An
    /// empty list for a root that is not an object.
    fn root_members(&mut self) -> Result<Vec<Member>, JsonError> {
        self.space();
        if self.byte(self.at) != Some(b'{') {
            return Ok(Vec::new());
        }
        let mut members = Vec::new();
        self.object(0, &mut |key: Text<'_>, value, elements| {
            members.push(Member {
                key: key.into_string(),
                value,
                elements,
            });
        })?;
        Ok(members)
    }

    /// Every element of the array at `array`, in order, handed to `keep` as its own text.
    ///
    /// The span came from a root member this walk already validated, so the only refusal
    /// that can come out is `keep`'s — and it wins over anything the re-walk finds, because
    /// a refusal is what the caller is waiting for and a syntax fault here would be a fault
    /// the first walk missed.
    pub(super) fn elements(
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
                let Some((from, to)) = span.bounds() else { return };
                let Some(element) = text.get(from..to) else { return };
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
    /// [`Self::elements`]: the text was validated by [`Document::new`], so a syntax fault
    /// here would be a fault that walk missed, and the caller is waiting on the refusal.
    pub(super) fn members(
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
    pub(super) fn fault(&self, what: &'static str) -> JsonError {
        self.fault_at(self.at, what)
    }

    /// `JsonError::Syntax` at `at`.
    pub(super) fn fault_at(&self, at: usize, what: &'static str) -> JsonError {
        JsonError::Syntax {
            at: u32::try_from(at).unwrap_or(u32::MAX),
            what,
        }
    }

    /// The byte at `at`, or `None` past the end.
    pub(super) fn byte(&self, at: usize) -> Option<u8> {
        self.text.as_bytes().get(at).copied()
    }

    /// Past the whitespace, where a value or a member name may start.
    pub(super) fn space(&mut self) {
        while matches!(self.byte(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    /// Eats `byte` if it is next, moving the cursor only when it was.
    pub(super) fn eat(&mut self, byte: u8) -> bool {
        let found = self.byte(self.at) == Some(byte);
        self.at += usize::from(found);
        found
    }

    /// One JSON value at the cursor, at `depth`: where it is, and the cursor just past it.
    pub(super) fn value(&mut self, depth: u32) -> Result<Span, JsonError> {
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
    fn literal(&mut self, word: &str) -> Result<(), JsonError> {
        if !self.text[self.at..].starts_with(word) {
            return Err(self.fault("not the start of a value"));
        }
        self.at += word.len();
        Ok(())
    }

    /// One array at `depth`, handing each element's span to `keep`.
    pub(super) fn array(
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
    fn object(
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
    fn key_seen(
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

/// A range as `u32`s, saturating rather than wrapping (D6). Nothing under the byte ceiling
/// can reach the saturation; a span that did is refused where it is read, not here.
fn span(start: usize, end: usize) -> Span {
    Span {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end: u32::try_from(end).unwrap_or(u32::MAX),
    }
}
