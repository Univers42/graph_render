//! Declared roles, read. The vocabulary is in `graph_contract::ingest`; this is the
//! motor's own view of a document through it.
//!
//! "The first field with this role" means the **lowest-id** field with it. The contract's
//! reader sorts a collection's fields by id on the way in, and this module sorts them
//! again rather than trusting that, because every `Ingest` field is `pub` and a document
//! built in Rust reaches here unsorted. So the order in which two adapters happened to
//! write their declarations cannot reach a derived graph.
//!
//! Every function here takes a **declared field id or a declared role** and returns what
//! that field's value says. None of them looks at a field's *name*, its position, or a
//! host's type string — which is the whole point of the contract. The code this
//! replaces reverse-engineered roles from vendor property types ("the first
//! `multi_select`, or a field named `/^tags?$/i`"), and a heuristic that picks the wrong
//! role **silently changes the graph**: there is no error, no warning, and nothing in
//! the output to detect it. A declared role cannot be wrong in that way, because the
//! declaration is the thing being read.
//!
//! It also fixes **H6**, which the phase prompt names: `groupValue` depended on
//! `Object.values()` ordering, which is not stable across a JSON reserialization. Here
//! every read is `record.value(declared_id)`, and the contract's reader sorts a
//! record's cells by key on the way in, so nothing downstream can observe their order.
//!
//! **Ponytail (absent vs empty).** A missing cell and a present-but-empty one are
//! different facts, and this module keeps them apart: `group` answers `None` for the
//! first and `Some("")` for the second. Direction: this is the *safe* direction — a
//! caller that conflates them gets a visibly empty group rather than a plausible wrong
//! one. Escape hatch: none is needed, because the distinction is already exposed; a
//! caller that wants "either absent or empty is no group" says so at its own call site
//! rather than having the contract decide it for every caller.

use graph_contract::ingest::{
    Cardinality, Collection, Field, Ingest, JsonValue, Link, Record, Role,
};

/// The weight a record with no `weight`-role value gets.
///
/// A **convention, and it is the one number in this module with no derivation behind
/// it.** 0.5 is the midpoint of the 0..1 range `NodeRecord` documents, chosen so an
/// unweighted record is neither invisible nor dominant. Direction: change it and every
/// layout of a dataset whose records carry no weight changes, with nothing in the output
/// saying so — the same global-silent failure mode as the strength table. Escape
/// hatch: a source that cares declares a `weight` role and the default never applies;
/// this constant is read only when a `weight` field's value is absent or not a number.
pub const DEFAULT_WEIGHT: f64 = 0.5;

/// The value of the collection's `titleField`, or `None`. This is the **only** place a
/// node's label comes from: the collection names the field, so no search is involved.
pub fn label<'a>(doc: &'a Ingest, record: &'a Record) -> Option<&'a str> {
    let collection = doc.collection(&record.collection)?;
    let field_id = collection.title_field.as_str();
    text(record, field_id)
}

/// The value of the collection's first `label`-role field as the node's `group`, or —
/// for a collection that declares no `label` role at all — the first `group`-role field.
///
/// **The choice is the collection's, not the record's.** Whether the fallback exists is
/// decided by the *declaration* — does this collection declare a `label` role? — and
/// never by whether one record happens to carry a cell for it. Deciding per record would
/// make the same collection derive two different node columns: a record whose label cell
/// is absent would silently take its `group` cell, and one whose label cell is present
/// would not, with nothing in the output saying which rule ran. `Role::Label` wins where
/// both are declared, because that is what the contract's `Role::Label` says it is ("a
/// second string, the node's `group`"); `Role::Group` is the fallback so that a document
/// declaring it is not silently dropped — a declared role with no reader is
/// indistinguishable from a role nobody implemented.
///
/// **Ponytail (two roles, one column).** Failing input: a collection declaring both a
/// `label` and a `group` field, where the `group` field's value is the one the user
/// expected to see. Direction: the `label` role wins, because the contract names it as the
/// node's group; the `group` value is dropped and nothing in the output says so. Escape
/// hatch: declare `label` on the field whose value should be the group.
pub fn group<'a>(doc: &'a Ingest, record: &'a Record) -> Option<&'a str> {
    let collection = doc.collection(&record.collection)?;
    let role = match role_field(collection, Role::Label) {
        Some(_) => Role::Label,
        None => Role::Group,
    };
    role_value(doc, record, role).and_then(JsonValue::as_text)
}

/// The values of the collection's first `tags`-role field, as strings, in the order
/// they were written. A value that is not a string is **skipped**, not stringified: a
/// number in a tags field is a schema mistake, and `"7"` would hide it.
pub fn tags(doc: &Ingest, record: &Record) -> Vec<String> {
    role_value(doc, record, Role::Tags)
        .and_then(JsonValue::as_list)
        .map(|items| {
            items
                .iter()
                .filter_map(JsonValue::as_text)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// The value of the collection's first `weight`-role field, or [`DEFAULT_WEIGHT`].
///
/// The number derives **as declared**, with no clamp and no refusal. `NodeRecord.weight`
/// documents 0..1 by convention, not by validation, and both committed source fixtures
/// declare 3, 5 and 8 — a clamp would silently rewrite every weight in the convergence
/// dataset (three distinct nodes all to `1.0`) and a refusal would make the phase's own
/// fixture unbuildable.
///
/// **Ponytail (an out-of-range weight).** Failing input: a source whose weight column is
/// a score (`-1`, `8`, or a percentage on a 0..100 scale). Direction: the value is passed
/// through, so a consumer that assumes 0..1 — a legend, a colour ramp, a radius — draws
/// outside its own scale rather than being told. Escape hatch: a source that wants the
/// convention declares a weight already inside it, or normalises before it declares.
pub fn weight(doc: &Ingest, record: &Record) -> f64 {
    role_value(doc, record, Role::Weight)
        .and_then(JsonValue::as_number)
        .unwrap_or(DEFAULT_WEIGHT)
}

/// The one record id a `parent`-role field names, or `None`.
///
/// Two shapes are read, and only two: a bare string, and a list of **exactly one**
/// string. The list form is there because a source whose references are always a
/// collection (`relation` properties, foreign-key columns) writes a one-element list
/// even where the relationship is one-to-one, and refusing that would make the contract
/// unable to express a tree from such a source at all. A list of **two or more** is
/// refused: that is several claims about a tree, and picking the first would silently
/// drop the others.
///
/// Direction: dropping a parent is the dangerous direction (a subtree detaches and
/// nothing in the output says so), so an unreadable value yields `None` and the record
/// derives no hierarchy edge at all. Escape hatch: a source with genuinely several
/// parents declares the relationship as a `link` role instead, which says plainly that
/// it is many.
pub fn parent<'a>(doc: &'a Ingest, record: &'a Record) -> Option<&'a str> {
    match role_value(doc, record, Role::Parent)? {
        JsonValue::Text(id) => Some(id),
        JsonValue::List(items) if items.len() == 1 => items[0].as_text(),
        _ => None,
    }
}

/// The record ids a `link`-role field names, in the order they were written.
///
/// The field's declared `cardinality` decides the shape, and only that shape: a `one`
/// field holds a single reference and a `many` field a list, and a value of the other
/// shape yields **nothing** rather than being read as a one-element list. Direction:
/// reading across the declared cardinality would turn a schema mistake into a graph
/// that looks right; escaping it hides a field that is doing nothing. Escape hatch: the
/// declaration is the fix — a source whose links arrive singly declares `one`.
pub fn references(doc: &Ingest, record: &Record, field_id: &str) -> Vec<String> {
    let Some(link) = link_of(doc, &record.collection, field_id) else {
        return Vec::new();
    };
    let Some(value) = record.value(field_id) else {
        return Vec::new();
    };
    match (link.cardinality, value) {
        (Cardinality::One, JsonValue::Text(id)) => vec![id.clone()],
        (Cardinality::Many, JsonValue::List(items)) => items
            .iter()
            .filter_map(JsonValue::as_text)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// The link a `link`-role field declares, or `None` if the field is not one. The
/// declaration is the only source: no inference, and a field with the wrong role
/// declares no link even if its value looks like a list of ids.
pub fn link_of<'a>(doc: &'a Ingest, collection_id: &str, field_id: &str) -> Option<&'a Link> {
    doc.collection(collection_id)?
        .field(field_id)?
        .link
        .as_ref()
}

/// Every `link`-role field of a collection, in the contract's canonical order (by field
/// id). The derived edge order therefore does not depend on how the document listed its
/// fields (H6), and a tie in any downstream sort falls to the id, never to insertion.
pub fn link_fields(collection: &Collection) -> impl Iterator<Item = &Field> {
    canonical(collection.fields.iter().filter(|f| f.role == Role::Link)).into_iter()
}

/// The first field with `role` in the contract's canonical order, or `None`. A
/// *declared* first, never a discovered one — and the lowest id, never the first written.
fn role_field(collection: &Collection, role: Role) -> Option<&Field> {
    canonical(collection.fields.iter().filter(|f| f.role == role))
        .into_iter()
        .next()
}

/// The collection's fields with one role, in canonical order: ascending by field id.
///
/// The contract's reader sorts a collection's fields (`validate::check`), but every
/// `Ingest` field is `pub`, so a document built in Rust — an adapter, a test, the SDK —
/// reaches this module with whatever order it was written in. Sorting here rather than
/// trusting the reader is what makes "the lowest-id field with this role" true on this
/// tree for every document, not only the ones that came out of `read()`.
///
/// The sort is **stable** and the key is the id, so two fields sharing an id (a schema
/// mistake the contract's reader refuses) keep their declared order among themselves and
/// every other case is decided by content alone (D3, D5).
fn canonical<'a>(fields: impl Iterator<Item = &'a Field>) -> Vec<&'a Field> {
    let mut ordered: Vec<&Field> = fields.collect();
    ordered.sort_by(|a, b| a.id.cmp(&b.id));
    ordered
}

/// The value of the first field declared with `role`, or `None`. The lifetime is the
/// document's, because the cell lives in the record the document owns; the record
/// borrow is only used to find the field id.
fn role_value<'a>(doc: &'a Ingest, record: &'a Record, role: Role) -> Option<&'a JsonValue> {
    let collection = doc.collection(&record.collection)?;
    let field_id = &role_field(collection, role)?.id;
    record.value(field_id)
}

fn text<'a>(record: &'a Record, field_id: &str) -> Option<&'a str> {
    record.value(field_id).and_then(JsonValue::as_text)
}
