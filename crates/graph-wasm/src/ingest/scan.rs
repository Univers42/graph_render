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
//!    the same byte offsets with the same messages. It locates the root's member list on
//!    the same pass — the whole text is read once for this, and the second pass that used
//!    to follow found nothing the first had not already validated.
//! 2. [`Scan::elements`] walks one array again, handing each element's text straight to the
//!    record reader. No index, no offsets kept, nothing to pre-size.
//!
//! Split in two because the reader this replaces parses the *entire* text before it looks at
//! the root, so a syntax fault in `edges[4000000]` is reported ahead of a missing `version`.
//! Locating the root inside the first walk does not disturb that: the shape refusals are
//! still raised by the caller, after this has had its say on the whole text. Folding the
//! *record* pass into the first walk would, which is why it is not done
//! (`docs/measurements/perf-p4d-extend.md`).
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
mod walk;

use graph_contract::canonical_json::JsonError;

use super::IngestError;
/// Deepest nesting read, as `graph_contract::canonical_json::parse` reads it.
pub(super) const MAX_DEPTH: u32 = 32;

/// Member count above which an object's duplicate-key check builds a set of the keys it has
/// read instead of scanning them — the same threshold, and for the same reason, as the
/// reader this replaces: a quadratic over a hostile object is a worse trade than a clone per
/// member.
pub(super) const WIDE_OBJECT: usize = 32;

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

/// One object member as the walk reads it: its unescaped key, where its value is, and —
/// for a value that is a string — that string's own text.
///
/// The text is the point. A record reader needs every string field's bytes unescaped, and
/// this walk has just read them; handing them over is what saves the reader from lexing
/// the same bytes a second time (`docs/measurements/perf-p4d-extend.md`). `None` for a
/// value that is not a string, which is every other kind of value there is.
pub(in crate::ingest) struct Field<'a> {
    pub(in crate::ingest) key: Text<'a>,
    pub(in crate::ingest) value: Span,
    pub(in crate::ingest) text: Option<Text<'a>>,
}

/// A validated ingest document: the root's members, and nothing per element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Document<'a> {
    text: &'a str,
    /// Whether the root was an object at all. A document whose root is an array or a number
    /// validates and locates no members, and the reader has to tell that apart from a
    /// document whose root is an object that is missing `version` — the two are different
    /// refusals, and which one comes out is the reader's published order.
    object: bool,
    members: Vec<Member>,
}

impl<'a> Document<'a> {
    /// `text` as strict RFC 8259 — refused exactly as
    /// [`graph_contract::canonical_json::parse`] refuses it — with the root's members
    /// located. A root that is not an object yields no members; [`read_records`](super)
    /// refuses that with a shape error of its own, after this has had its say on the syntax.
    pub(super) fn new(text: &'a str) -> Result<Self, JsonError> {
        let mut scan = Scan::new(text);
        let (object, members) = scan.root()?;
        Ok(Self {
            text,
            object,
            members,
        })
    }

    /// The document's text, so a reader that already holds it is not handed it twice.
    pub(super) fn text(&self) -> &'a str {
        self.text
    }

    /// Whether the root is an object. `false` is a shape fault the reader names, and it is
    /// named before any member is looked at — which is only knowable here, because the walk
    /// validates the whole text first and an empty member list is what a non-object root and
    /// an empty object both leave behind.
    pub(super) fn is_object(&self) -> bool {
        self.object
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
}

/// A range as `u32`s, saturating rather than wrapping (D6). Nothing under the byte ceiling
/// can reach the saturation; a span that did is refused where it is read, not here.
pub(super) fn span(start: usize, end: usize) -> Span {
    Span {
        start: u32::try_from(start).unwrap_or(u32::MAX),
        end: u32::try_from(end).unwrap_or(u32::MAX),
    }
}
