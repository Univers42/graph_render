//! The two passes that read a whole ingest document: [`Scan::root`], which validates all
//! of it and locates the root's members in the same walk, and [`Scan::records`], which
//! reads one root member's array and each element's members with it.
//!
//! Split from [`super`] for the house line limit. Both are `Scan` methods, so
//! `Scan`'s fields are the parent's and these two share its cursor.

use graph_contract::canonical_json::JsonError;

use crate::ingest::element::Element;

use crate::ingest::IngestError;

use super::super::{Field, Member, Span};
use super::Scan;

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
        self.space();
        if self.eat(b'[') && !self.eat(b']') {
            self.elements(element, keep, &mut refused)?;
        }
        match refused {
            Some(why) => Err(why),
            None => Ok(()),
        }
    }

    /// The elements of the array the cursor is just inside, the last refusal among them in
    /// `refused`, and the array's own shape: brackets, commas, closing.
    ///
    /// This is [`Scan::array`] with the element walk inlined, because `array` reads each
    /// element as a *value* first and the record pass must read it as an object's members
    /// instead — the same bytes, once. Ten lines of `array`'s body are therefore spelled out
    /// here rather than shared, and `array` keeps its own copy for every other caller.
    fn elements(
        &mut self,
        element: &mut Element<'a>,
        keep: &mut impl FnMut(&mut Element<'a>) -> Result<(), IngestError>,
        refused: &mut Option<IngestError>,
    ) -> Result<(), IngestError> {
        // The element's position in the list, advanced only once an element has been read
        // into a record. A refused element leaves it where it was, so the element after it
        // is refused at the *same* position — which is what the reader that numbered the
        // elements in its own loop did, and what the frozen reader names.
        let mut index = 0usize;
        loop {
            self.space();
            let from = self.at;
            element.seek(index);
            element.reset();
            match self.read_element(from, element, keep) {
                Ok(Err(why)) => *refused = Some(why),
                // A fault in the element's own bytes stops the walk, as reading the element
                // as one value did. Its offset is moved onto the element, because that is
                // where the pass this replaces counted from.
                Err(fault) => {
                    *refused = Some(IngestError::Json(rebase(fault, from)));
                    return Ok(());
                }
                Ok(Ok(())) => index += 1,
            }
            self.space();
            if self.eat(b']') {
                return Ok(());
            }
            if !self.eat(b',') {
                return Err(IngestError::Json(self.fault("expected , or ] in an array")));
            }
        }
    }

    /// The element at `from`, read into `element`: the refusal it raised, or `Ok(())`,
    /// or — for a fault in the element's own bytes — that fault, which stops the array.
    ///
    /// The same three outcomes as [`Element::fill`], with the record built on top of the
    /// members it located. `keep` is asked only once the members are in, and its refusal is
    /// the same kind: an `Err` inside an `Ok`.
    fn read_element(
        &mut self,
        from: usize,
        element: &mut Element<'a>,
        keep: &mut impl FnMut(&mut Element<'a>) -> Result<(), IngestError>,
    ) -> Result<Result<(), IngestError>, JsonError> {
        match element.fill(self)? {
            Err(why) => Ok(Err(why)),
            Ok(()) => match self.text.get(from..self.at) {
                Some(_) => Ok(keep(element)),
                // A span the walk produced that does not fit the text it walked cannot
                // happen; the element is skipped rather than read as an empty record, which
                // is what the pass that handed out element texts did with the same span.
                None => Ok(Ok(())),
            },
        }
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
