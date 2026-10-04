//! One record element's members, held as the walk located them and read on demand.
//!
//! Split from [`super`](super) for the house line limit: the field names and the record
//! builders are the parent's, the table is this module's.
//!
//! A table is built once per list and refilled per element — it is 300 bytes of stack, and
//! clearing it costs less than a fresh frame per record.
//!
//! # Precondition
//!
//! Every member is filled by [`Element::fill`], which the array walk calls on one object it
//! has just validated. Every refusal the accessors raise is therefore a *shape* fault — a
//! member the shape does not name, one that is missing, or one of the wrong type — never a
//! syntax fault.

use super::field;
use crate::ingest::at::At;
use crate::ingest::scan::{Field, Scan, Span, Text};
use crate::ingest::{IngestError, shape};
use graph_contract::canonical_json::JsonError;
use graph_core::{EdgeKind, NodeKind};

/// The widest record, so one stack table serves both shapes.
const WIDEST: usize = super::NODE_FIELDS.len();

/// One record element's members, held as the walk located them.
pub(in crate::ingest) struct Element<'a> {
    /// What `spans` index into: the whole document, so one walk's spans slice directly.
    text: &'a str,
    fields: &'static [&'static str],
    spans: [Option<Span>; WIDEST],
    /// The walk's own unescaped text for each member that is a string, and `None` for
    /// every other kind of value and for a member that was not written.
    quoted: [Option<Text<'a>>; WIDEST],
    /// The list this element belongs to, with its own index once `seek` has run.
    at: At,
}

impl<'a> Element<'a> {
    /// An empty table for `fields`, of the list `at`.
    pub(in crate::ingest) fn new(text: &'a str, fields: &'static [&'static str], at: At) -> Self {
        Self {
            text,
            fields,
            spans: [None; WIDEST],
            quoted: core::array::from_fn(|_| None),
            at,
        }
    }

    /// Points the table at element `index`, so every refusal it raises names that element.
    pub(in crate::ingest) fn seek(&mut self, index: usize) {
        self.at = self.at.item(index);
    }

    /// Where the element in the table sits, for a refusal that names it.
    pub(in crate::ingest) fn at(&self) -> At {
        self.at
    }

    /// Forgets the last element's members, so the walk can fill this table again.
    ///
    /// `quoted` is *not* cleared, and does not need to be: every accessor checks
    /// [`Self::span`] before it reads that slot, so a member this element did not write is
    /// refused as missing and never reaches a stale text. An owned (escaped) string left in
    /// a slot is freed when the next element writes over it, or when the table is dropped —
    /// which is the same `Text` the old table dropped per field anyway.
    pub(in crate::ingest) fn reset(&mut self) {
        self.spans = [None; WIDEST];
    }

    /// The members of the object the cursor is on, into this table.
    ///
    /// `Ok(Err(why))` is a refusal about the shape — an unknown key, raised in document
    /// order before any member is read, and the last one raised within this element, which
    /// is what the pass over the element's members this replaces did. `Err(fault)` is the
    /// walk refusing the element's own bytes, which stops the array.
    pub(in crate::ingest) fn fill(
        &mut self,
        scan: &mut Scan<'a>,
    ) -> Result<Result<(), IngestError>, JsonError> {
        let mut refused: Option<IngestError> = None;
        let walked = scan.object(0, &mut |member: Field<'a>, _| {
            if let Err(why) = self.member(member) {
                refused = Some(why);
            }
        });
        match refused {
            Some(why) => Ok(Err(why)),
            None => walked.map(|()| Ok(())),
        }
    }

    /// One member in the offset its name holds, or the refusal a name the shape does not
    /// list gets. Checked for every member of the element before any of them is read, so a
    /// stray key is a loud refusal and not a dropped extra.
    fn member(&mut self, member: Field<'a>) -> Result<(), IngestError> {
        let at = self.at;
        let key = member.key.as_str();
        let index = self
            .fields
            .iter()
            .position(|name| *name == key)
            .ok_or_else(|| shape(at, &format!("unknown member `{key}`")))?;
        self.spans[index] = Some(member.value);
        self.quoted[index] = member.text;
        Ok(())
    }

    /// Where field `field` is, or the refusal a member that is not there gets.
    fn span(&self, field: usize, name: &'static str) -> Result<Span, IngestError> {
        self.spans[field].ok_or_else(|| shape(self.at, &format!("missing member `{name}`")))
    }

    /// [`Self::span`] for a member that may be absent: whether it is there.
    pub(super) fn written(&self, field: usize) -> bool {
        self.spans[field].is_some()
    }

    /// The text of `span`, or `None` if the span does not fit the document — which the walk
    /// that produced it cannot produce, and which is refused rather than sliced.
    fn slice(&self, span: Span) -> Option<&'a str> {
        let (from, to) = span.bounds()?;
        self.text.get(from..to)
    }

    /// Field `field`'s own text as the walk read it, or `None` for a member of any other
    /// type.
    ///
    /// The walk has already read these bytes, so there is no second lexical pass over them
    /// and no way for this to disagree with what the walk saw: a member the walk did not
    /// read as a string carries no text here either, and is refused as not a string.
    fn quoted(&mut self, field: usize) -> Option<Text<'a>> {
        self.quoted[field].take()
    }

    /// Field `field` as a string, refusing a member of any other type at
    /// `at.field(name)`.
    pub(super) fn string(
        &mut self,
        field: usize,
        name: &'static str,
    ) -> Result<String, IngestError> {
        self.span(field, name)?;
        self.quoted(field)
            .map(Text::into_string)
            .ok_or_else(|| shape(self.at.field(name), "expected a string"))
    }

    /// Field `field` as a string or an explicit `null`.
    pub(super) fn opt_string(
        &mut self,
        field: usize,
        name: &'static str,
    ) -> Result<Option<String>, IngestError> {
        let span = self.span(field, name)?;
        if self.slice(span) == Some("null") {
            return Ok(None);
        }
        // Worded here rather than through `Self::string`: a member that may be absent
        // refuses a wrong type by naming both the types it accepts, which is not what a
        // member that must be a string says.
        self.quoted(field)
            .map(|text| Some(text.into_string()))
            .ok_or_else(|| shape(self.at.field(name), "expected a string or null"))
    }

    /// Field `field` as a boolean.
    pub(super) fn boolean(
        &mut self,
        field: usize,
        name: &'static str,
    ) -> Result<bool, IngestError> {
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
    pub(super) fn number(&mut self, field: usize, name: &'static str) -> Result<f64, IngestError> {
        let span = self.span(field, name)?;
        let at = self.at.field(name);
        let text = self
            .slice(span)
            .ok_or_else(|| shape(at, "not a valid number"))?;
        if !matches!(text.as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
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
    pub(super) fn node_kind(&mut self, at: At) -> Result<NodeKind, IngestError> {
        let name = self.kind_text(field::NODE_KIND)?;
        NodeKind::from_name(name.as_str()).ok_or_else(|| {
            shape(
                at.field("kind"),
                &format!("unknown node kind {:?}", name.as_str()),
            )
        })
    }

    /// [`Self::node_kind`] for an edge.
    pub(super) fn edge_kind(&mut self, at: At) -> Result<EdgeKind, IngestError> {
        let name = self.kind_text(field::EDGE_KIND)?;
        EdgeKind::from_name(name.as_str()).ok_or_else(|| {
            shape(
                at.field("kind"),
                &format!("unknown edge kind {:?}", name.as_str()),
            )
        })
    }

    /// Field `field` as a string, or the refusal for it missing or not a string.
    fn kind_text(&mut self, field: usize) -> Result<Text<'a>, IngestError> {
        self.span(field, "kind")?;
        self.quoted(field)
            .ok_or_else(|| shape(self.at.field("kind"), "expected a string"))
    }
}
