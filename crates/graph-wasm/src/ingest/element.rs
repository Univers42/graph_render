//! One node or edge element, read out of the document's own text.
//!
//! The reader this replaces took the element as a `canonical_json::Value` and moved each
//! string out of it; this one takes the element's text and reads its members in place. The
//! two agree member for member: every key is checked against the shape *before* any member
//! is read (so a stray `hasNote` is a loud refusal, not a dropped extra), `kind` is read
//! before any other member, the rest are read in field order whatever order the document
//! wrote them in, and an edge's optional `child_first` is read last.
//!
//! A record's members land in a fixed-size `[Option<Span>; 10]` on the stack — one pass over
//! the element, nothing allocated per member — and are still *read* in the fixed order
//! afterwards, which is what makes a node with two bad members refuse for the same one a
//! reader that built a `Value` refused for.

use super::at::At;
use super::scan::{Scan, Span, Text};
use super::{IngestError, shape};
use graph_core::{EdgeKind, EdgeRecord, NodeKind, NodeRecord};

/// Field names `node` requires, exactly. Module-level rather than a `let` inside the
/// function (house limit: the array was most of what pushed it past 40 lines).
const NODE_FIELDS: [&str; 10] = [
    "id",
    "kind",
    "database_id",
    "source",
    "label",
    "group",
    "weight",
    "version",
    "has_note",
    "icon",
];

const EDGE_FIELDS: [&str; 9] = [
    "id",
    "source",
    "target",
    "kind",
    "label",
    "strength",
    "directed",
    "record_id",
    "child_first",
];

/// Field offsets, named so a struct literal reads as the field list does. `kind` is 1 in
/// both shapes and is read first; the rest follow in the order the shape names them.
mod field {
    pub(super) const ID: usize = 0;
    pub(super) const KIND: usize = 1;
    pub(super) const SOURCE: usize = 2;
    pub(super) const TARGET: usize = 3;
    pub(super) const LABEL: usize = 4;
    pub(super) const STRENGTH: usize = 5;
    pub(super) const DIRECTED: usize = 6;
    pub(super) const RECORD_ID: usize = 7;
    pub(super) const CHILD_FIRST: usize = 8;
    pub(super) const DATABASE_ID: usize = 2;
    pub(super) const GROUP: usize = 5;
    pub(super) const WEIGHT: usize = 6;
    pub(super) const VERSION: usize = 7;
    pub(super) const HAS_NOTE: usize = 8;
    pub(super) const ICON: usize = 9;
}

/// The widest record, so one stack array serves both shapes.
const WIDEST: usize = NODE_FIELDS.len();

/// The node element written in `text`, or the refusal naming `at`.
pub(super) fn node(text: &str, at: At) -> Result<NodeRecord, IngestError> {
    let mut element = Element::read(text, &NODE_FIELDS, at)?;
    // `kind` first, exactly as the reader that built a `Value` read it: a node naming no
    // known kind is refused before any other member of it is looked at.
    let kind = element.node_kind(at)?;
    Ok(NodeRecord {
        id: element.string(field::ID, "id")?,
        kind,
        database_id: element.opt_string(field::DATABASE_ID, "database_id")?,
        source: element.string(field::SOURCE, "source")?,
        label: element.string(field::LABEL, "label")?,
        group: element.opt_string(field::GROUP, "group")?,
        weight: element.number(field::WEIGHT, "weight")?,
        version: element.number(field::VERSION, "version")?,
        has_note: element.boolean(field::HAS_NOTE, "has_note")?,
        icon: element.opt_string(field::ICON, "icon")?,
    })
}

/// The edge element written in `text`, or the refusal naming `at`.
pub(super) fn edge(text: &str, at: At) -> Result<EdgeRecord, IngestError> {
    let mut element = Element::read(text, &EDGE_FIELDS, at)?;
    let kind = element.edge_kind(at)?;
    Ok(EdgeRecord {
        id: element.string(field::ID, "id")?,
        source: element.string(field::SOURCE, "source")?,
        target: element.string(field::TARGET, "target")?,
        kind,
        label: element.string(field::LABEL, "label")?,
        strength: element.number(field::STRENGTH, "strength")?,
        directed: element.boolean(field::DIRECTED, "directed")?,
        record_id: element.opt_string(field::RECORD_ID, "record_id")?,
        // Optional, and read last in field position: an edge written before p3's hierarchy
        // direction reads as parent-first, the same default as `graph-cli`'s
        // `oracle_fixtures/wire.rs`. An edge with both a bad `directed` and a bad
        // `child_first` is refused for `directed`.
        child_first: match element.written(field::CHILD_FIRST) {
            true => element.boolean(field::CHILD_FIRST, "child_first")?,
            false => false,
        },
    })
}

/// One record's members, held as spans and read on demand.
///
/// # Precondition
///
/// `text` is one whole object value that a [`Scan`] has already validated: it comes from
/// [`Scan::elements`](super::scan::Scan::elements), so it is well-formed and complete.
/// Every refusal here is therefore a *shape* fault — a member the shape does not name, one
/// that is missing, or one of the wrong type — never a syntax fault.
struct Element<'a> {
    text: &'a str,
    spans: [Option<Span>; WIDEST],
    at: At,
}

impl<'a> Element<'a> {
    /// Every member of the object written in `text`, checked against `fields` in document
    /// order. A member the shape does not name is refused here, before any member is read.
    fn read(text: &'a str, fields: &[&'static str], at: At) -> Result<Self, IngestError> {
        let mut element = Self {
            text,
            spans: [None; WIDEST],
            at,
        };
        let Self { spans, .. } = &mut element;
        let mut scan = Scan::new(text);
        scan.members(&mut |key, span| {
            let field = fields
                .iter()
                .position(|name| *name == key.as_str())
                .ok_or_else(|| shape(at, &format!("unknown member `{}`", key.as_str())))?;
            spans[field] = Some(span);
            Ok(())
        })?;
        Ok(element)
    }

    /// Where field `field` is, or the refusal a member that is not there gets.
    fn span(&self, field: usize, name: &str) -> Result<Span, IngestError> {
        self.spans[field].ok_or_else(|| shape(self.at, &format!("missing member `{name}`")))
    }

    /// [`Self::span`] for a member that may be absent: whether it is there.
    fn written(&self, field: usize) -> bool {
        self.spans[field].is_some()
    }

    /// The text of `span`, or `None` if the span does not fit the element — which the walk
    /// that produced it cannot produce, and which is refused rather than sliced.
    fn slice(&self, span: Span) -> Option<&'a str> {
        let (from, to) = span.bounds()?;
        self.text.get(from..to)
    }

    /// The member at `span` as a string, refusing a member of any other type at
    /// `at.field(name)`.
    fn text_at(&self, span: Span, name: &str) -> Result<Text<'a>, IngestError> {
        if self.slice(span).is_none_or(|text| !text.starts_with('"')) {
            return Err(shape(self.at.field(name), "expected a string"));
        }
        let mut scan = Scan::new(self.text);
        let (text, _) = scan
            .string(span.start as usize)
            .map_err(IngestError::Json)?;
        Ok(text)
    }

    /// Whether the member at `span` is the literal `null`.
    fn is_null(&self, span: Span) -> bool {
        self.slice(span) == Some("null")
    }

    /// Field `field` as a string.
    fn string(&mut self, field: usize, name: &str) -> Result<String, IngestError> {
        let span = self.span(field, name)?;
        self.text_at(span, name).map(Text::into_string)
    }

    /// Field `field` as a string or an explicit `null`.
    fn opt_string(&mut self, field: usize, name: &str) -> Result<Option<String>, IngestError> {
        let span = self.span(field, name)?;
        match self.is_null(span) {
            true => Ok(None),
            false => self.text_at(span, name).map(Text::into_string).map(Some),
        }
    }

    /// Field `field` as a boolean.
    fn boolean(&mut self, field: usize, name: &str) -> Result<bool, IngestError> {
        let at = self.at.field(name);
        match self.slice(self.span(field, name)?) {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err(shape(at, "expected a boolean")),
        }
    }

    /// Field `field` as a finite `f64` (D9). The text is the document's own, so the JSON
    /// grammar has already had its say; what is left is an exponent large enough to
    /// overflow, which parses as `inf` and must still be refused here.
    fn number(&mut self, field: usize, name: &str) -> Result<f64, IngestError> {
        let span = self.span(field, name)?;
        let at = self.at.field(name);
        let text = self.slice(span).ok_or_else(|| shape(at, "not a valid number"))?;
        if !text.starts_with(['-', '0'..='9']) {
            return Err(shape(at, "not a valid number"));
        }
        let value: f64 = text.parse().map_err(|_| shape(at, "not a valid number"))?;
        if !value.is_finite() {
            return Err(shape(at, "not finite"));
        }
        Ok(value)
    }

    /// `kind`, read before every other member and refused by name when it names no kind
    /// this build knows.
    fn node_kind(&mut self, at: At) -> Result<NodeKind, IngestError> {
        let span = self.span(field::KIND, "kind")?;
        let name = self.text_at(span, "kind")?;
        NodeKind::from_name(name.as_str()).ok_or_else(|| {
            shape(at.field("kind"), &format!("unknown node kind {:?}", name.as_str()))
        })
    }

    /// [`Self::node_kind`] for an edge.
    fn edge_kind(&mut self, at: At) -> Result<EdgeKind, IngestError> {
        let span = self.span(field::KIND, "kind")?;
        let name = self.text_at(span, "kind")?;
        EdgeKind::from_name(name.as_str()).ok_or_else(|| {
            shape(at.field("kind"), &format!("unknown edge kind {:?}", name.as_str()))
        })
    }
}
